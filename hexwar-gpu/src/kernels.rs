//! CUDA kernel definitions for GPU game simulation
//!
//! These kernels are compiled at runtime using NVRTC.
//! Each thread simulates one complete game using random move selection.

/// CUDA kernel source code
/// Compiled at runtime for the target GPU architecture
pub const KERNEL_SOURCE: &str = r#"
// Kernel constants matching Rust compact.rs
#define BOARD_SIZE 61
#define MAX_LEGAL_MOVES 128

// Piece type constants
#define PIECE_EMPTY 255
#define PIECE_TYPE_MASK 0xFF

// Move types
#define MOVE_PASS 0
#define MOVE_MOVEMENT 1
#define MOVE_ROTATE 2
#define MOVE_SWAP 3
#define MOVE_REBIRTH 4
#define MOVE_INVALID 255

// Game results
#define RESULT_ONGOING 0
#define RESULT_WHITE_WINS 1
#define RESULT_BLACK_WINS 2

// Direction vectors in axial coordinates (dq, dr)
// Index: 0=N, 1=NE, 2=SE, 3=S, 4=SW, 5=NW
__constant__ char DIRECTIONS[6][2] = {
    {0, -1},  // N
    {1, -1},  // NE
    {1, 0},   // SE
    {0, 1},   // S
    {-1, 1},  // SW
    {-1, 0}   // NW
};

// Piece type definitions (simplified for GPU)
// Each piece has: move_type, range, direction_mask, is_king
// Move types: 0=Step, 1=Slide, 2=Jump, 3=None
__constant__ unsigned char PIECE_DEFS[30][4] = {
    // Step-1 pieces (A1-A5)
    {0, 1, 0x01, 0}, // A1 Pawn: Step-1, Forward
    {0, 1, 0x3F, 0}, // A2 Guard: Step-1, All
    {0, 1, 0x23, 0}, // A3 Scout: Step-1, Forward arc
    {0, 1, 0x2C, 0}, // A4 Crab: Step-1, FL+FR+B
    {0, 1, 0x22, 0}, // A5 Flanker: Step-1, FL+FR
    // Step-2 pieces (B1-B4)
    {0, 2, 0x01, 0}, // B1 Strider
    {0, 2, 0x22, 0}, // B2 Dancer
    {0, 2, 0x3F, 0}, // B3 Ranger
    {0, 2, 0x23, 0}, // B4 Hound
    // Step-3 pieces (C1-C3)
    {0, 3, 0x01, 0}, // C1 Lancer
    {0, 3, 0x23, 0}, // C2 Dragoon
    {0, 3, 0x3F, 0}, // C3 Courser
    // Slide pieces (D1-D5)
    {1, 99, 0x01, 0}, // D1 Pike
    {1, 99, 0x09, 0}, // D2 Rook
    {1, 99, 0x36, 0}, // D3 Bishop
    {1, 99, 0x23, 0}, // D4 Chariot
    {1, 99, 0x3F, 0}, // D5 Queen
    // Jump pieces (E1-F2)
    {2, 2, 0x23, 0}, // E1 Knight
    {2, 2, 0x3F, 0}, // E2 Frog
    {2, 3, 0x23, 0}, // F1 Locust
    {2, 3, 0x3F, 0}, // F2 Cricket
    // Special pieces (W1-G1)
    {3, 0, 0x00, 0}, // W1 Warper
    {0, 1, 0x3F, 0}, // W2 Shifter
    {0, 1, 0x23, 0}, // P1 Phoenix
    {0, 1, 0x3F, 0}, // G1 Ghost
    // Kings (K1-K5)
    {0, 1, 0x3F, 1}, // K1 King Guard
    {0, 1, 0x23, 1}, // K2 King Scout
    {0, 2, 0x3F, 1}, // K3 King Ranger
    {2, 2, 0x3F, 1}, // K4 King Frog
    {1, 99, 0x01, 1}  // K5 King Pike
};

// Compact piece: 2 bytes
struct CompactPiece {
    unsigned char piece_type;
    unsigned char packed; // bits 0-2: facing, bit 3: owner
};

// Compact move: 4 bytes
struct CompactMove {
    unsigned char move_type;
    unsigned char from_idx;
    unsigned char to_idx;
    unsigned char facing;
};

// Simulation result: 8 bytes
struct SimulationResult {
    unsigned char result;
    unsigned char rounds;
    short final_eval_x100;
    unsigned char padding[4];
};

// Simple XorShift RNG (per-thread state)
__device__ unsigned int xorshift32(unsigned int* state) {
    unsigned int x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    return x;
}

// Check if hex coordinates are valid (radius 4 board)
__device__ bool is_valid_hex(int q, int r) {
    return (q >= -4 && q <= 4 &&
            r >= -4 && r <= 4 &&
            (q + r) >= -4 && (q + r) <= 4);
}

// Convert hex to board index
__device__ int hex_to_index(int q, int r) {
    if (!is_valid_hex(q, r)) return -1;

    int idx = 0;
    for (int prev_q = -4; prev_q < q; prev_q++) {
        int r_min = max(-4, -4 - prev_q);
        int r_max = min(4, 4 - prev_q);
        idx += r_max - r_min + 1;
    }
    int r_min = max(-4, -4 - q);
    idx += r - r_min;
    return idx;
}

// Convert index to hex coordinates
__device__ void index_to_hex(int idx, int* q, int* r) {
    if (idx < 0 || idx >= BOARD_SIZE) {
        *q = 99; *r = 99; // Invalid
        return;
    }

    int remaining = idx;
    for (int qq = -4; qq <= 4; qq++) {
        int r_min = max(-4, -4 - qq);
        int r_max = min(4, 4 - qq);
        int row_size = r_max - r_min + 1;

        if (remaining < row_size) {
            *q = qq;
            *r = r_min + remaining;
            return;
        }
        remaining -= row_size;
    }
    *q = 99; *r = 99;
}

// Get absolute direction from facing + relative direction
__device__ int abs_direction(int facing, int rel_dir) {
    return (facing + rel_dir) % 6;
}

// Check if a king is captured (game over condition)
__device__ int check_king_capture(CompactPiece* board) {
    bool white_king_exists = false;
    bool black_king_exists = false;

    for (int i = 0; i < BOARD_SIZE; i++) {
        if (board[i].piece_type != PIECE_EMPTY && board[i].piece_type < 30) {
            bool is_king = PIECE_DEFS[board[i].piece_type][3] != 0;
            if (is_king) {
                int owner = (board[i].packed >> 3) & 1;
                if (owner == 0) white_king_exists = true;
                else black_king_exists = true;
            }
        }
    }

    if (!white_king_exists) return RESULT_BLACK_WINS;
    if (!black_king_exists) return RESULT_WHITE_WINS;
    return RESULT_ONGOING;
}

// Generate legal moves for current player (simplified version)
// Returns count of moves written to moves array
__device__ int generate_moves(
    CompactPiece* board,
    int current_player,
    CompactMove* moves
) {
    int count = 0;

    // Always can pass
    moves[count].move_type = MOVE_PASS;
    moves[count].from_idx = 255;
    moves[count].to_idx = 255;
    moves[count].facing = 0;
    count++;

    // Iterate through board
    for (int idx = 0; idx < BOARD_SIZE && count < MAX_LEGAL_MOVES - 1; idx++) {
        CompactPiece piece = board[idx];
        if (piece.piece_type == PIECE_EMPTY || piece.piece_type >= 30) continue;

        int owner = (piece.packed >> 3) & 1;
        if (owner != current_player) continue;

        int facing = piece.packed & 7;
        unsigned char move_type = PIECE_DEFS[piece.piece_type][0];
        unsigned char range = PIECE_DEFS[piece.piece_type][1];
        unsigned char dir_mask = PIECE_DEFS[piece.piece_type][2];

        int q, r;
        index_to_hex(idx, &q, &r);

        // Generate rotation moves (6 possible facings)
        for (int new_facing = 0; new_facing < 6 && count < MAX_LEGAL_MOVES - 1; new_facing++) {
            if (new_facing != facing) {
                moves[count].move_type = MOVE_ROTATE;
                moves[count].from_idx = idx;
                moves[count].to_idx = idx;
                moves[count].facing = new_facing;
                count++;
            }
        }

        // Generate movement moves based on piece type
        if (move_type == 3) continue; // Warper can't move normally

        for (int rel_dir = 0; rel_dir < 6 && count < MAX_LEGAL_MOVES - 1; rel_dir++) {
            if (!(dir_mask & (1 << rel_dir))) continue;

            int abs_dir = abs_direction(facing, rel_dir);
            int dq = DIRECTIONS[abs_dir][0];
            int dr = DIRECTIONS[abs_dir][1];

            // Generate moves along this direction
            int max_dist = (move_type == 2) ? range : range; // Jump or step/slide

            for (int dist = 1; dist <= max_dist && count < MAX_LEGAL_MOVES - 1; dist++) {
                int tq = q + dq * dist;
                int tr = r + dr * dist;

                if (!is_valid_hex(tq, tr)) break;

                int to_idx = hex_to_index(tq, tr);
                CompactPiece target = board[to_idx];

                // For jump, only exact distance
                if (move_type == 2 && dist != range) continue;

                // Check if blocked (for step/slide)
                if (move_type != 2 && dist < range) {
                    if (target.piece_type != PIECE_EMPTY) {
                        // Can capture enemy, but stop here
                        int target_owner = (target.packed >> 3) & 1;
                        if (target_owner != current_player) {
                            // Capture move - try all facings
                            for (int nf = 0; nf < 6 && count < MAX_LEGAL_MOVES - 1; nf++) {
                                moves[count].move_type = MOVE_MOVEMENT;
                                moves[count].from_idx = idx;
                                moves[count].to_idx = to_idx;
                                moves[count].facing = nf;
                                count++;
                            }
                        }
                        break; // Blocked either way
                    }
                }

                // Empty or enemy (for jump, or end of range)
                if (target.piece_type == PIECE_EMPTY) {
                    // Move to empty - try all facings
                    for (int nf = 0; nf < 6 && count < MAX_LEGAL_MOVES - 1; nf++) {
                        moves[count].move_type = MOVE_MOVEMENT;
                        moves[count].from_idx = idx;
                        moves[count].to_idx = to_idx;
                        moves[count].facing = nf;
                        count++;
                    }
                } else {
                    int target_owner = (target.packed >> 3) & 1;
                    if (target_owner != current_player) {
                        // Capture
                        for (int nf = 0; nf < 6 && count < MAX_LEGAL_MOVES - 1; nf++) {
                            moves[count].move_type = MOVE_MOVEMENT;
                            moves[count].from_idx = idx;
                            moves[count].to_idx = to_idx;
                            moves[count].facing = nf;
                            count++;
                        }
                    }
                    if (move_type == 2) break; // Jump can capture but we're done
                }
            }
        }
    }

    return count;
}

// Apply a move to the board
__device__ void apply_move(
    CompactPiece* board,
    CompactMove move,
    int* current_player
) {
    if (move.move_type == MOVE_PASS) {
        // Just switch player
        *current_player = 1 - *current_player;
        return;
    }

    if (move.move_type == MOVE_ROTATE) {
        // Update facing
        board[move.from_idx].packed = (board[move.from_idx].packed & 0xF8) | (move.facing & 0x07);
        *current_player = 1 - *current_player;
        return;
    }

    if (move.move_type == MOVE_MOVEMENT) {
        // Move piece, possibly capturing
        board[move.to_idx] = board[move.from_idx];
        board[move.to_idx].packed = (board[move.to_idx].packed & 0xF8) | (move.facing & 0x07);
        board[move.from_idx].piece_type = PIECE_EMPTY;
        board[move.from_idx].packed = 0;
        *current_player = 1 - *current_player;
        return;
    }

    if (move.move_type == MOVE_SWAP) {
        // Swap two pieces
        CompactPiece temp = board[move.from_idx];
        board[move.from_idx] = board[move.to_idx];
        board[move.to_idx] = temp;
        *current_player = 1 - *current_player;
        return;
    }

    // Other move types not fully implemented
    *current_player = 1 - *current_player;
}

// Simple evaluation function (material + center control)
__device__ int simple_eval(CompactPiece* board) {
    int score = 0;

    for (int i = 0; i < BOARD_SIZE; i++) {
        if (board[i].piece_type == PIECE_EMPTY || board[i].piece_type >= 30) continue;

        int owner = (board[i].packed >> 3) & 1;
        int sign = (owner == 0) ? 1 : -1; // White positive, Black negative

        // Piece value (simplified)
        int piece_val = 10;
        bool is_king = PIECE_DEFS[board[i].piece_type][3] != 0;
        if (is_king) piece_val = 1000;

        score += sign * piece_val;

        // Center control bonus
        int q, r;
        index_to_hex(i, &q, &r);
        int dist_center = (abs(q) + abs(r) + abs(q + r)) / 2;
        score += sign * (4 - dist_center);
    }

    return score;
}

// Main simulation kernel
// Each thread simulates one complete game
extern "C" __global__ void simulate_games(
    CompactPiece* initial_states,  // [batch_size][BOARD_SIZE]
    unsigned char* initial_players, // [batch_size]
    unsigned int* seeds,           // [batch_size] - RNG seeds
    unsigned int max_moves,
    SimulationResult* results      // [batch_size]
) {
    int tid = blockIdx.x * blockDim.x + threadIdx.x;

    // Copy initial state to local memory
    CompactPiece board[BOARD_SIZE];
    for (int i = 0; i < BOARD_SIZE; i++) {
        board[i] = initial_states[tid * BOARD_SIZE + i];
    }

    int current_player = initial_players[tid];
    unsigned int rng_state = seeds[tid];

    // Storage for legal moves
    CompactMove moves[MAX_LEGAL_MOVES];

    unsigned int round = 0;
    int result = RESULT_ONGOING;

    // Simulate game
    while (round < max_moves && result == RESULT_ONGOING) {
        // Generate legal moves
        int move_count = generate_moves(board, current_player, moves);

        if (move_count == 0) {
            // No moves available (shouldn't happen, we always have pass)
            break;
        }

        // Pick random move
        unsigned int rnd = xorshift32(&rng_state);
        int move_idx = rnd % move_count;

        // Apply move
        apply_move(board, moves[move_idx], &current_player);

        // Check for game over
        result = check_king_capture(board);

        round++;
    }

    // If game didn't end, use evaluation to determine "winner"
    if (result == RESULT_ONGOING && round >= max_moves) {
        int eval = simple_eval(board);
        if (eval > 50) result = RESULT_WHITE_WINS;
        else if (eval < -50) result = RESULT_BLACK_WINS;
        // else stays ongoing (draw-ish)
    }

    // Write results
    results[tid].result = result;
    results[tid].rounds = (round > 255) ? 255 : round;

    int eval = simple_eval(board);
    if (eval > 32767) eval = 32767;
    if (eval < -32768) eval = -32768;
    results[tid].final_eval_x100 = eval;
}
"#;

/// Name of the main simulation kernel
pub const KERNEL_NAME: &str = "simulate_games";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_source_not_empty() {
        assert!(!KERNEL_SOURCE.is_empty());
        assert!(KERNEL_SOURCE.contains("simulate_games"));
    }
}
