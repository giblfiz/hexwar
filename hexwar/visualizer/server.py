#!/usr/bin/env python3
"""
Simple web server for HEXWAR board visualization and designer.
"""

import json
import http.server
import socketserver
from pathlib import Path
from datetime import datetime
import sys
import threading
import time

# Add parent to path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from hexwar.pieces import PIECE_TYPES, REGULAR_PIECE_IDS, KING_IDS
from hexwar.board import ALL_HEXES, BOARD_RADIUS, DIRECTIONS, DIRECTION_NAMES
from hexwar.game import GameState, Move, apply_move, generate_legal_actions
from hexwar.ai import Heuristics
import random

# Try to import Rust engine for faster AI
try:
    # The local hexwar_core/ directory shadows the installed package,
    # so we need to load the .so file directly
    import importlib.util
    import glob

    # Find the .so file in the virtual environment or system site-packages
    so_pattern = "**/hexwar_core/hexwar_core.cpython-*.so"
    project_root = Path(__file__).parent.parent.parent
    venv_path = project_root / '.venv'

    so_paths = list(venv_path.glob(so_pattern)) if venv_path.exists() else []
    if not so_paths:
        # Try system paths
        import site
        for sp in site.getsitepackages():
            so_paths = list(Path(sp).glob("hexwar_core/hexwar_core.cpython-*.so"))
            if so_paths:
                break

    if not so_paths:
        raise ImportError("hexwar_core .so not found")

    so_path = str(so_paths[0])
    # Module name must match what's compiled into the .so (PyInit_hexwar_core)
    spec = importlib.util.spec_from_file_location('hexwar_core', so_path)
    _rust_module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(_rust_module)
    rust_get_ai_move = _rust_module.get_ai_move
    rust_play_game_with_record = _rust_module.play_game_with_record

    print(f"Rust AI engine loaded from {so_path}")
except Exception as e:
    raise RuntimeError(f"Rust AI engine required but not available: {e}. Rebuild with: cd hexwar_core && maturin develop --release")

PORT = 8002
STATIC_DIR = Path(__file__).parent

# Designer state (shared across requests)
designer_state = {
    'pieces': [],
    'graveyard': {'white': [], 'black': []},
    'templates': {'white': 'E', 'black': 'E'},
    'version': 0,
    'name': '',  # Board name shown in UI text box
}
designer_lock = threading.Lock()

# Game playback state
playback_state = {
    'record': None,  # GameRecord object
    'player': None,  # GamePlayer object
    'move_index': 0,
    'total_moves': 0,
    'active': False,
}
playback_lock = threading.RLock()  # Reentrant lock for nested calls

# Interactive game state
game_state = {
    'active': False,
    'state': None,  # GameState object
    'player_side': 0,  # 0=white, 1=black
    'ai_depth': 4,
    'ruleset': None,  # Ruleset dict
}
game_lock = threading.Lock()

# History log for all designer positions
HISTORY_FILE = Path(__file__).parent.parent.parent / 'designer_history.jsonl'


class HexwarHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(STATIC_DIR), **kwargs)

    def do_GET(self):
        if self.path == '/api/pieces':
            self.send_json(self.get_pieces_data())
        elif self.path == '/api/board':
            self.send_json(self.get_board_data())
        elif self.path.startswith('/api/champion/'):
            name = self.path.split('/')[-1]
            self.send_json(self.get_champion_data(name))
        elif self.path == '/api/champions':
            self.send_json(self.get_champions_list())
        elif self.path == '/api/seeds':
            self.send_json(self.get_seeds_list())
        elif self.path.startswith('/api/seed/'):
            name = self.path.split('/')[-1]
            self.send_json(self.get_seed_data(name))
        elif self.path == '/api/designer':
            self.send_json(self.get_designer_state())
        elif self.path.startswith('/api/designer/poll'):
            self.send_json(self.poll_designer_state())
        elif self.path == '/api/playback/state':
            self.send_json(self.get_playback_state())
        elif self.path == '/api/piece-types':
            self.send_json(self.get_pieces_data())
        elif self.path == '/api/rulesets':
            self.send_json(self.get_rulesets())
        elif self.path == '/':
            # Redirect to designer
            self.send_response(302)
            self.send_header('Location', '/designer.html')
            self.end_headers()
        else:
            super().do_GET()

    def do_POST(self):
        content_length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_length).decode('utf-8')

        if self.path == '/api/designer':
            self.send_json(self.update_designer_state(body))
        elif self.path == '/api/designer/load':
            self.send_json(self.load_into_designer(body))
        elif self.path == '/api/playback/load':
            self.send_json(self.load_game_record(body))
        elif self.path == '/api/playback/forward':
            self.send_json(self.playback_forward())
        elif self.path == '/api/playback/backward':
            self.send_json(self.playback_backward())
        elif self.path == '/api/playback/goto':
            self.send_json(self.playback_goto(body))
        elif self.path == '/api/playback/stop':
            self.send_json(self.playback_stop())
        elif self.path == '/api/game/start':
            self.send_json(self.start_game(body))
        elif self.path == '/api/game/move':
            self.send_json(self.make_player_move(body))
        elif self.path == '/api/game/ai-move':
            self.send_json(self.get_ai_move(body))
        else:
            self.send_response(404)
            self.end_headers()

    def send_json(self, data):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def get_pieces_data(self):
        """Return all piece type definitions."""
        pieces = {}
        for pid in list(REGULAR_PIECE_IDS) + list(KING_IDS):
            pt = PIECE_TYPES[pid]
            pieces[pid] = {
                'id': pt.id,
                'name': pt.name,
                'move_type': pt.move_type,
                'move_range': pt.move_range if pt.move_range < 100 else 9,
                'directions': list(pt.directions),
                'special': pt.special,
                'is_king': pt.is_king,
            }
        return pieces

    def get_board_data(self):
        """Return board geometry."""
        return {
            'radius': BOARD_RADIUS,
            'hexes': list(ALL_HEXES),
            'directions': list(DIRECTIONS),
            'direction_names': list(DIRECTION_NAMES),
        }

    def get_champions_list(self):
        """Return list of saved champions from the d7 exotics run."""
        base_dir = Path(__file__).parent.parent.parent
        champions = []

        # Show d7 exotics run champions
        run_dir = base_dir / 'balance_jan08_d7_exotics_0603'
        champions_dir = run_dir / 'champions'
        if champions_dir.exists():
            for f in sorted(champions_dir.glob('*.json')):
                with open(f) as fp:
                    data = json.load(fp)
                ucb = data.get('ucb_score', 0)
                champions.append({
                    'name': f.stem,
                    'run': run_dir.name,
                    'id': f'{run_dir.name}/{f.stem}',
                    'ucb': ucb
                })

        # Sort by UCB descending
        champions.sort(key=lambda x: -x['ucb'])
        return champions

    def get_champion_data(self, name):
        """Return a specific champion's ruleset. Name can be 'run/champion' or just 'champion'."""
        base_dir = Path(__file__).parent.parent.parent

        # Check if name includes run (e.g., 'balance_jan06_0851/some-champion')
        if '/' in name:
            run, champ_name = name.split('/', 1)
            champion_file = base_dir / run / 'champions' / f'{champ_name}.json'
            if champion_file.exists():
                with open(champion_file) as f:
                    data = json.load(f)
                    data['run'] = run
                    return data

        # Search all runs and seed directories for this champion name
        for pattern in ['balance_*', 'seeds_*']:
            for run_dir in sorted(base_dir.glob(pattern), reverse=True):
                champion_file = run_dir / 'champions' / f'{name}.json'
                if champion_file.exists():
                    with open(champion_file) as f:
                        data = json.load(f)
                        data['run'] = run_dir.name
                        return data

        # Also check board_sets/ directory
        board_sets_file = base_dir / 'board_sets' / f'{name}.json'
        if board_sets_file.exists():
            with open(board_sets_file) as f:
                data = json.load(f)
                data['run'] = 'board_sets'
                return data

        # Check board_sets subdirectories (like d7_seeds/)
        for subdir in (base_dir / 'board_sets').glob('*'):
            if subdir.is_dir():
                board_file = subdir / f'{name}.json'
                if board_file.exists():
                    with open(board_file) as f:
                        data = json.load(f)
                        data['run'] = f'board_sets/{subdir.name}'
                        return data

        return {'error': f'Champion not found: {name}'}

    def get_seeds_list(self):
        """Return list of seeds from seeds_d6_exotics/champions."""
        base_dir = Path(__file__).parent.parent.parent
        seeds = []

        seeds_dir = base_dir / 'seeds_d6_exotics' / 'champions'
        if seeds_dir.exists():
            for f in sorted(seeds_dir.glob('*.json')):
                with open(f) as fp:
                    data = json.load(fp)
                ucb = data.get('ucb_score', 0)
                seeds.append({
                    'name': f.stem,
                    'ucb': ucb
                })

        # Sort by UCB descending
        seeds.sort(key=lambda x: -x['ucb'])
        return [s['name'] for s in seeds]

    def get_seed_data(self, name):
        """Return a seed configuration from seeds_d6_exotics/champions."""
        base_dir = Path(__file__).parent.parent.parent
        seed_file = base_dir / 'seeds_d6_exotics' / 'champions' / f'{name}.json'

        if seed_file.exists():
            with open(seed_file) as f:
                data = json.load(f)
                data['name'] = f'[seed] {name}'
                return data

        return {'error': f'Seed not found: {name}'}

    def get_designer_state(self):
        """Return current designer state."""
        with designer_lock:
            return dict(designer_state)

    def poll_designer_state(self):
        """Long-poll for designer updates. Returns if version changed."""
        # Parse version from query string
        query = self.path.split('?')[-1] if '?' in self.path else ''
        params = dict(p.split('=') for p in query.split('&') if '=' in p)
        client_version = int(params.get('version', 0))

        # Wait up to 5 seconds for an update (short for responsiveness)
        for _ in range(50):  # 50 * 0.1s = 5 seconds
            with designer_lock:
                # Reload if server version changed OR if server restarted (version < client)
                if designer_state['version'] != client_version:
                    return {'reload': True, **designer_state}
            time.sleep(0.1)

        return {'reload': False, 'version': designer_state['version']}

    def update_designer_state(self, body):
        """Update designer state from client."""
        global designer_state
        try:
            data = json.loads(body)
            with designer_lock:
                designer_state['pieces'] = data.get('pieces', [])
                designer_state['graveyard'] = data.get('graveyard', {'white': [], 'black': []})
                designer_state['templates'] = data.get('templates', {'white': 'E', 'black': 'E'})
                if 'name' in data:
                    designer_state['name'] = data['name']
                designer_state['version'] += 1
                version = designer_state['version']

            # Log to history file (include name for context)
            data['board_name'] = designer_state.get('name', '')
            self.log_designer_history(data)

            return {'success': True, 'version': version}
        except Exception as e:
            return {'error': str(e)}

    def load_into_designer(self, body):
        """Load a champion/seed into the designer."""
        global designer_state
        try:
            data = json.loads(body)
            name = data.get('name', '')

            # Find the champion or seed
            champion_data = self.get_champion_data(name)
            if 'error' in champion_data:
                # Try seeds
                champion_data = self.get_seed_data(name)

            if 'error' in champion_data:
                return champion_data

            # Convert ruleset to designer format
            ruleset = champion_data.get('ruleset', {})
            pieces = []

            # Get facings arrays (index 0 = king, 1+ = pieces)
            white_facings = ruleset.get('white_facings', [])
            black_facings = ruleset.get('black_facings', [])

            # Add white king
            if ruleset.get('white_positions') and ruleset.get('white_king'):
                pieces.append({
                    'id': 1,
                    'pieceId': ruleset['white_king'],
                    'color': 'white',
                    'pos': ruleset['white_positions'][0],
                    'facing': white_facings[0] if white_facings else 0
                })

            # Add white pieces
            white_pieces = ruleset.get('white_pieces', [])
            white_positions = ruleset.get('white_positions', [])[1:]
            for i, (pid, pos) in enumerate(zip(white_pieces, white_positions)):
                facing = white_facings[i + 1] if white_facings and i + 1 < len(white_facings) else 0
                pieces.append({
                    'id': 100 + i,
                    'pieceId': pid,
                    'color': 'white',
                    'pos': pos,
                    'facing': facing
                })

            # Add black king
            if ruleset.get('black_positions') and ruleset.get('black_king'):
                pieces.append({
                    'id': 2,
                    'pieceId': ruleset['black_king'],
                    'color': 'black',
                    'pos': ruleset['black_positions'][0],
                    'facing': black_facings[0] if black_facings else 3
                })

            # Add black pieces
            black_pieces = ruleset.get('black_pieces', [])
            black_positions = ruleset.get('black_positions', [])[1:]
            for i, (pid, pos) in enumerate(zip(black_pieces, black_positions)):
                facing = black_facings[i + 1] if black_facings and i + 1 < len(black_facings) else 3
                pieces.append({
                    'id': 200 + i,
                    'pieceId': pid,
                    'color': 'black',
                    'pos': pos,
                    'facing': facing
                })

            with designer_lock:
                designer_state['pieces'] = pieces
                designer_state['graveyard'] = {'white': [], 'black': []}
                designer_state['templates'] = {
                    'white': ruleset.get('white_template', 'E'),
                    'black': ruleset.get('black_template', 'E')
                }
                designer_state['name'] = name
                designer_state['version'] += 1

            # Log to history
            self.log_designer_history({'pieces': pieces, 'loaded_from': name})

            return {'success': True, 'version': designer_state['version'], 'loaded': name}
        except Exception as e:
            return {'error': str(e)}

    def log_designer_history(self, data):
        """Append a timestamped entry to the history log."""
        try:
            entry = {
                'timestamp': datetime.now().isoformat(),
                'data': data
            }
            with open(HISTORY_FILE, 'a') as f:
                f.write(json.dumps(entry) + '\n')
        except Exception as e:
            print(f"Error logging history: {e}")

    # ========== Game Playback Methods ==========

    def get_playback_state(self):
        """Return current playback state for the UI."""
        with playback_lock:
            if not playback_state['active'] or playback_state['player'] is None:
                return {'active': False}

            player = playback_state['player']
            state = player.state

            # Convert game state to pieces format for UI
            pieces = []
            for pos, piece in state.board.items():
                pieces.append({
                    'id': id(piece),
                    'pieceId': piece.type_id,
                    'color': 'white' if piece.owner == 0 else 'black',
                    'pos': list(pos),
                    'facing': piece.facing,
                })

            return {
                'active': True,
                'move_index': player.move_index,
                'total_moves': player.total_moves,
                'at_start': player.at_start,
                'at_end': player.at_end,
                'current_player': state.current_player,
                'round_number': state.round_number,
                'winner': playback_state['record'].winner if player.at_end else None,
                'pieces': pieces,
            }

    def load_game_record(self, body):
        """Load a game record for playback.

        Accepts either:
        - {"path": "/path/to/game.json"} - load from server-side file
        - {"record": {...}} - inline game record dict
        - {...} - direct game record (if has 'moves' key)
        """
        global playback_state
        try:
            from hexwar.game_record import GameRecord, GamePlayer

            data = json.loads(body)

            # Can load from file path, nested record, or direct record
            if 'path' in data:
                record = GameRecord.from_file(data['path'])
            elif 'record' in data:
                record = GameRecord.from_dict(data['record'])
            elif 'moves' in data:
                # Direct game record JSON
                record = GameRecord.from_dict(data)
            else:
                return {'error': 'Must provide "path", "record", or direct game record with "moves"'}

            player = GamePlayer(record)

            with playback_lock:
                playback_state['record'] = record
                playback_state['player'] = player
                playback_state['move_index'] = 0
                playback_state['total_moves'] = record.num_moves()
                playback_state['active'] = True

            return {
                'success': True,
                'total_moves': record.num_moves(),
                'winner': record.winner,
                'end_reason': record.end_reason,
            }
        except Exception as e:
            return {'error': str(e)}

    def playback_forward(self):
        """Step forward one move in playback."""
        with playback_lock:
            if not playback_state['active'] or playback_state['player'] is None:
                return {'error': 'No active playback'}

            player = playback_state['player']
            result = player.forward()

            if result is None:
                return {'error': 'Already at end', 'at_end': True}

            playback_state['move_index'] = player.move_index
            return self.get_playback_state()

    def playback_backward(self):
        """Step backward one move in playback."""
        with playback_lock:
            if not playback_state['active'] or playback_state['player'] is None:
                return {'error': 'No active playback'}

            player = playback_state['player']
            result = player.backward()

            if result is None:
                return {'error': 'Already at start', 'at_start': True}

            playback_state['move_index'] = player.move_index
            return self.get_playback_state()

    def playback_goto(self, body):
        """Jump to specific move index."""
        with playback_lock:
            if not playback_state['active'] or playback_state['player'] is None:
                return {'error': 'No active playback'}

            try:
                data = json.loads(body)
                move_index = int(data.get('index', 0))
            except:
                return {'error': 'Invalid index'}

            player = playback_state['player']
            player.goto(move_index)
            playback_state['move_index'] = player.move_index

            return self.get_playback_state()

    def playback_stop(self):
        """Stop playback mode."""
        global playback_state
        with playback_lock:
            playback_state = {
                'record': None,
                'player': None,
                'move_index': 0,
                'total_moves': 0,
                'active': False,
            }
        return {'success': True}

    # ========== Interactive Game Methods ==========

    def get_rulesets(self):
        """Return list of available rulesets."""
        return [
            {
                'id': 'copper-pass',
                'name': 'copper-pass',
                'path': str(Path(__file__).parent.parent.parent / 'board_sets' / 'd7_firstarrow_seeds' / 'copper-pass.json'),
            }
        ]

    def start_game(self, body):
        """Start a new interactive game."""
        global game_state
        try:
            data = json.loads(body)
            ruleset_path = data.get('ruleset', 'default')
            player_side_str = data.get('player_side', 'white')
            ai_depth = int(data.get('ai_depth', 4))

            player_side = 0 if player_side_str == 'white' else 1

            # Load ruleset - look up path from rulesets list
            rulesets = {r['id']: r for r in self.get_rulesets()}
            if ruleset_path in rulesets and 'path' in rulesets[ruleset_path]:
                with open(rulesets[ruleset_path]['path']) as f:
                    ruleset_data = json.load(f)
                    ruleset = ruleset_data.get('ruleset', ruleset_data)
            elif ruleset_path == 'default':
                ruleset = self._default_ruleset()
            else:
                # Try as direct file path
                with open(ruleset_path) as f:
                    ruleset_data = json.load(f)
                    ruleset = ruleset_data.get('ruleset', ruleset_data)

            # Create initial game state
            white_pieces, black_pieces = self._ruleset_to_piece_lists(ruleset)
            white_template = ruleset.get('white_template', 'E')
            black_template = ruleset.get('black_template', 'E')

            state = GameState.create_initial(
                white_pieces, black_pieces,
                white_template, black_template
            )

            with game_lock:
                game_state['active'] = True
                game_state['state'] = state
                game_state['player_side'] = player_side
                game_state['ai_depth'] = ai_depth
                game_state['ruleset'] = ruleset

            # Get legal moves for current player
            legal_moves = self._get_legal_moves_json(state)

            return {
                'state': self._state_to_json(state),
                'legal_moves': legal_moves
            }
        except Exception as e:
            import traceback
            traceback.print_exc()
            return {'error': str(e)}

    def make_player_move(self, body):
        """Apply a player's move to the game."""
        global game_state
        try:
            data = json.loads(body)
            move_data = data.get('move', {})

            with game_lock:
                if not game_state['active']:
                    return {'error': 'No active game'}

                state = game_state['state']

                # Convert move JSON to Move object
                move = self._json_to_move(move_data, state)
                if move is None:
                    return {'error': 'Invalid move format'}

                # Apply the move
                new_state = apply_move(state, move)
                game_state['state'] = new_state

                # Get legal moves for next player
                legal_moves = self._get_legal_moves_json(new_state)

                return {
                    'state': self._state_to_json(new_state),
                    'legal_moves': legal_moves
                }
        except Exception as e:
            import traceback
            traceback.print_exc()
            return {'error': str(e)}

    def get_ai_move(self, body):
        """Get the AI's move for the current position."""
        global game_state
        try:
            data = json.loads(body)
            depth = int(data.get('depth', game_state.get('ai_depth', 4)))

            with game_lock:
                if not game_state['active']:
                    return {'error': 'No active game'}

                state = game_state['state']

                if state.winner is not None:
                    return {'error': 'Game already finished'}

                heuristics = Heuristics.create_default()
                move = self._get_rust_ai_move(state, depth, heuristics)

                if move is None:
                    # No valid move - pass
                    move = Move(action_type='PASS', from_pos=None, to_pos=None, new_facing=None)

                # Apply the move
                new_state = apply_move(state, move)
                game_state['state'] = new_state

                # Get legal moves for next player
                legal_moves = self._get_legal_moves_json(new_state)

                return {
                    'state': self._state_to_json(new_state),
                    'legal_moves': legal_moves,
                    'move': self._move_to_json(move)
                }
        except Exception as e:
            import traceback
            traceback.print_exc()
            return {'error': str(e)}

    def _get_rust_ai_move(self, state, depth, heuristics):
        """Get AI move using Rust engine."""
        # Convert state to format for Rust
        pieces = []
        for pos, piece in state.board.items():
            pieces.append((piece.type_id, pos, piece.facing, piece.owner))

        heuristics_dict = {
            'white_piece_values': heuristics.white_piece_values,
            'black_piece_values': heuristics.black_piece_values,
            'white_center_weight': heuristics.white_center_weight,
            'black_center_weight': heuristics.black_center_weight,
        }

        # GameState uses templates tuple: (white_template, black_template)
        white_template, black_template = state.templates

        result = rust_get_ai_move(
            pieces=pieces,
            current_player=state.current_player,
            white_template=white_template,
            black_template=black_template,
            action_index=state.action_index,
            depth=depth,
            heuristics_dict=heuristics_dict,
            max_moves_per_action=15,
            seed=random.randint(0, 2**32),
        )

        if result is None:
            return None

        action_type, from_pos, to_pos, new_facing = result
        return Move(
            action_type=action_type,
            from_pos=tuple(from_pos) if from_pos else None,
            to_pos=tuple(to_pos) if to_pos else None,
            new_facing=new_facing
        )

    def _default_ruleset(self):
        """Create a simple default ruleset for testing."""
        return {
            'white_king': 'K1',
            'black_king': 'K1',
            'white_pieces': ['D1', 'D1', 'E1', 'A1', 'A1'],
            'black_pieces': ['D1', 'D1', 'E1', 'A1', 'A1'],
            'white_positions': [(0, 3), (-1, 3), (1, 3), (0, 4), (-2, 4), (2, 4)],
            'black_positions': [(0, -3), (-1, -3), (1, -3), (0, -4), (-2, -4), (2, -4)],
            'white_facings': [0, 0, 0, 0, 0, 0],
            'black_facings': [3, 3, 3, 3, 3, 3],
            'white_template': 'E',
            'black_template': 'E',
        }

    def _d10_endgame_ruleset(self):
        """The D10 bug game position at move 63 - where AI was making suicidal moves."""
        # From test_ai_correctness.py test_recreate_d10_endgame
        # White King at (1, 1), Black D1 at (0, -3) can slide to capture at (0, 1)
        return {
            'white_king': 'K2',
            'black_king': 'K1',
            'white_pieces': ['D2', 'C1', 'A5', 'F2', 'D2'],
            'black_pieces': ['G1', 'D1', 'P1', 'P1'],
            'white_positions': [(1, 1), (-4, 4), (-2, 1), (0, 3), (1, 0), (1, 3)],
            'black_positions': [(1, -2), (-1, 0), (0, -3), (2, -2), (3, -2)],
            'white_facings': [0, 0, 1, 0, 0, 0],
            'black_facings': [3, 4, 3, 3, 3],
            'white_template': 'E',
            'black_template': 'E',
        }

    def _ruleset_to_piece_lists(self, ruleset):
        """Convert ruleset dict to piece lists for GameState.create_initial."""
        white_pieces = []
        black_pieces = []

        # White king
        white_king = ruleset.get('white_king', 'K1')
        white_positions = ruleset.get('white_positions', [])
        white_facings = ruleset.get('white_facings', [])

        if white_positions:
            facing = white_facings[0] if white_facings else 0
            white_pieces.append((white_king, tuple(white_positions[0]), facing))

        # White regular pieces
        for i, pid in enumerate(ruleset.get('white_pieces', [])):
            pos_idx = i + 1  # +1 because king is at index 0
            if pos_idx < len(white_positions):
                facing = white_facings[pos_idx] if pos_idx < len(white_facings) else 0
                white_pieces.append((pid, tuple(white_positions[pos_idx]), facing))

        # Black king
        black_king = ruleset.get('black_king', 'K1')
        black_positions = ruleset.get('black_positions', [])
        black_facings = ruleset.get('black_facings', [])

        if black_positions:
            facing = black_facings[0] if black_facings else 3
            black_pieces.append((black_king, tuple(black_positions[0]), facing))

        # Black regular pieces
        for i, pid in enumerate(ruleset.get('black_pieces', [])):
            pos_idx = i + 1
            if pos_idx < len(black_positions):
                facing = black_facings[pos_idx] if pos_idx < len(black_facings) else 3
                black_pieces.append((pid, tuple(black_positions[pos_idx]), facing))

        return white_pieces, black_pieces

    def _state_to_json(self, state):
        """Convert GameState to JSON-serializable dict."""
        pieces = []
        for pos, piece in state.board.items():
            pieces.append({
                'pieceId': piece.type_id,
                'color': 'white' if piece.owner == 0 else 'black',
                'pos': list(pos),
                'facing': piece.facing,
            })

        return {
            'current_player': state.current_player,
            'round_number': state.round_number,
            'current_action': state.current_action,
            'winner': state.winner,
            'pieces': pieces,
        }

    def _get_legal_moves_json(self, state):
        """Get legal moves as JSON-serializable list."""
        if state.winner is not None:
            return []

        moves = generate_legal_actions(state)
        return [self._move_to_json(m) for m in moves]

    def _move_to_json(self, move):
        """Convert Move to JSON-serializable dict."""
        result = {
            'action_type': move.action_type,
            'from_pos': list(move.from_pos) if move.from_pos else None,
            'to_pos': list(move.to_pos) if move.to_pos else None,
            'new_facing': move.new_facing,
        }
        if move.special_data:
            result['special_data'] = move.special_data
        return result

    def _json_to_move(self, data, state):
        """Convert JSON move data to Move object."""
        action_type = data.get('action_type')
        if not action_type:
            return None

        from_pos = tuple(data['from_pos']) if data.get('from_pos') else None
        to_pos = tuple(data['to_pos']) if data.get('to_pos') else None
        new_facing = data.get('new_facing')
        special_data = data.get('special_data')

        return Move(
            action_type=action_type,
            from_pos=from_pos,
            to_pos=to_pos,
            new_facing=new_facing,
            special_data=special_data
        )


class ThreadedTCPServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    """Multi-threaded TCP server for handling concurrent requests."""
    allow_reuse_address = True
    daemon_threads = True  # Don't block exit on pending threads


def run_server():
    with ThreadedTCPServer(("", PORT), HexwarHandler) as httpd:
        print(f"HEXWAR Designer running at http://localhost:{PORT}")
        print("Press Ctrl+C to stop")
        httpd.serve_forever()


def load_champion_to_designer(name: str) -> dict:
    """
    Load a champion or seed into the designer.
    Call this from Claude to push a board state to the UI.

    Example:
        from hexwar.visualizer.server import load_champion_to_designer
        load_champion_to_designer('dual-ridge')
    """
    import requests
    try:
        res = requests.post(
            f'http://localhost:{PORT}/api/designer/load',
            json={'name': name},
            timeout=5
        )
        return res.json()
    except Exception as e:
        return {'error': str(e)}


def get_designer_state() -> dict:
    """
    Get the current designer state.
    Call this from Claude to see what the user has set up.

    Example:
        from hexwar.visualizer.server import get_designer_state
        state = get_designer_state()
    """
    import requests
    try:
        res = requests.get(f'http://localhost:{PORT}/api/designer', timeout=5)
        return res.json()
    except Exception as e:
        return {'error': str(e)}


def get_designer_history(last_n: int = 10) -> list:
    """
    Get the last N entries from the designer history.
    """
    entries = []
    if HISTORY_FILE.exists():
        with open(HISTORY_FILE) as f:
            for line in f:
                try:
                    entries.append(json.loads(line))
                except:
                    pass
    return entries[-last_n:]


if __name__ == '__main__':
    run_server()
