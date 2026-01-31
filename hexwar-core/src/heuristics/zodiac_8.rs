//! Zodiac Heuristics Set 8: Omega, Apex, Zenith
//!
//! Final refinement heuristics attempting to exceed Cronus.
//! Focus on optimal parameter balance.
//!
//! - OMEGA: Even higher material, lower center
//! - APEX: Cronus-like but with optimized piece ratios
//! - ZENITH: Maximum everything approach

use crate::eval::Heuristics;

/// OMEGA Heuristics (Ultimate Material)
///
/// Philosophy: "Material is everything"
///
/// Pushes material values even higher than Cronus.
/// Lower center weight to compensate.
pub fn omega_heuristics() -> Heuristics {
    let mut values = [1.0f32; 32];

    // EXTREME piece values
    values[0] = 5.0;   // Pawn
    values[1] = 10.0;  // Guard
    values[2] = 7.0;   // Scout
    values[3] = 7.0;   // Crab
    values[4] = 5.5;   // Flanker

    values[5] = 8.0;   // Strider
    values[6] = 9.0;   // Dancer
    values[7] = 14.0;  // Ranger
    values[8] = 10.0;  // Hound

    values[9] = 10.0;  // Lancer
    values[10] = 14.0; // Dragoon
    values[11] = 16.0; // Courser

    values[12] = 12.0; // Pike
    values[13] = 14.0; // Rook
    values[14] = 14.0; // Bishop
    values[15] = 15.0; // Chariot
    values[16] = 20.0; // Queen

    values[17] = 10.0; // Knight
    values[18] = 11.0; // Frog
    values[19] = 11.0; // Locust
    values[20] = 12.0; // Cricket

    values[21] = 9.0;  // Warper
    values[22] = 9.5;  // Shifter
    values[23] = 9.0;  // Phoenix
    values[24] = 7.0;  // Ghost

    values[25..30].fill(0.0);  // Kings

    // Trident pieces (added Jan 2026)
    values[30] = 9.0;   // Triton - Step-2, 3 trident dirs (like Dancer+)
    values[31] = 12.0;  // Triskelion - Slider, 3 trident dirs (like Bishop-)

    Heuristics {
        piece_values: values,
        center_weight: 1.0,      // Lower to balance high material
        mobility_weight: 0.01,   // Minimal
    }
}

/// APEX Heuristics (Optimized Ratios)
///
/// Philosophy: "Perfect balance of all factors"
///
/// Based on Cronus but with tweaked piece ratios.
/// Theory: Some pieces might be over/undervalued.
pub fn apex_heuristics() -> Heuristics {
    let mut values = [1.0f32; 32];

    // Optimized ratios - emphasize high-mobility pieces more
    values[0] = 3.0;   // Pawn - less valuable
    values[1] = 9.0;   // Guard - VERY valuable (all dirs)
    values[2] = 5.0;   // Scout
    values[3] = 5.0;   // Crab
    values[4] = 3.5;   // Flanker

    values[5] = 6.0;   // Strider
    values[6] = 7.0;   // Dancer
    values[7] = 13.0;  // Ranger - PREMIUM (all dirs step-2)
    values[8] = 8.0;   // Hound

    values[9] = 8.0;   // Lancer
    values[10] = 11.0; // Dragoon
    values[11] = 15.0; // Courser - PREMIUM (all dirs step-3)

    values[12] = 9.0;  // Pike
    values[13] = 11.0; // Rook
    values[14] = 11.0; // Bishop
    values[15] = 12.0; // Chariot
    values[16] = 17.0; // Queen

    values[17] = 8.0;  // Knight
    values[18] = 10.0; // Frog - valued for all-dir jump
    values[19] = 9.0;  // Locust
    values[20] = 11.0; // Cricket - valued for all-dir jump

    values[21] = 7.0;  // Warper
    values[22] = 7.5;  // Shifter
    values[23] = 7.0;  // Phoenix
    values[24] = 5.0;  // Ghost

    values[25..30].fill(0.0);  // Kings

    // Trident pieces (added Jan 2026)
    values[30] = 7.0;   // Triton - Step-2, 3 trident dirs
    values[31] = 11.0;  // Triskelion - Slider, 3 trident dirs

    Heuristics {
        piece_values: values,
        center_weight: 1.25,     // Between Cronus and Colossus
        mobility_weight: 0.02,   // Minimal
    }
}

/// ZENITH Heuristics (Maximum Everything)
///
/// Philosophy: "Go big or go home"
///
/// Maximum piece values, maximum center weight.
/// Theory: If both are good, maybe max both is best?
pub fn zenith_heuristics() -> Heuristics {
    let mut values = [1.0f32; 32];

    // MAXIMUM piece values
    values[0] = 5.0;   // Pawn
    values[1] = 10.0;  // Guard
    values[2] = 6.5;   // Scout
    values[3] = 6.5;   // Crab
    values[4] = 5.0;   // Flanker

    values[5] = 7.5;   // Strider
    values[6] = 8.5;   // Dancer
    values[7] = 14.0;  // Ranger
    values[8] = 10.0;  // Hound

    values[9] = 10.0;  // Lancer
    values[10] = 13.0; // Dragoon
    values[11] = 16.0; // Courser

    values[12] = 11.0; // Pike
    values[13] = 13.0; // Rook
    values[14] = 13.0; // Bishop
    values[15] = 14.0; // Chariot
    values[16] = 19.0; // Queen

    values[17] = 9.5;  // Knight
    values[18] = 10.5; // Frog
    values[19] = 10.5; // Locust
    values[20] = 12.0; // Cricket

    values[21] = 8.0;  // Warper
    values[22] = 9.0;  // Shifter
    values[23] = 8.5;  // Phoenix
    values[24] = 6.0;  // Ghost

    values[25..30].fill(0.0);  // Kings

    // Trident pieces (added Jan 2026)
    values[30] = 8.5;   // Triton - Step-2, 3 trident dirs (like Dancer)
    values[31] = 12.0;  // Triskelion - Slider, 3 trident dirs (like Bishop-)

    Heuristics {
        piece_values: values,
        center_weight: 1.5,      // High like Titan
        mobility_weight: 0.0,    // Zero like Titan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_omega_extreme_material() {
        let h = omega_heuristics();
        assert!(h.piece_values[16] >= 20.0, "Omega Queen should be max");
        assert!(h.center_weight <= 1.0, "Omega center should be lower");
    }

    #[test]
    fn test_apex_optimized() {
        let h = apex_heuristics();
        // All-direction pieces should be premium
        assert!(h.piece_values[7] >= 13.0, "Apex Ranger should be premium");
        assert!(h.piece_values[11] >= 15.0, "Apex Courser should be premium");
    }

    #[test]
    fn test_zenith_maximum() {
        let h = zenith_heuristics();
        assert!(h.center_weight >= 1.5, "Zenith should have high center");
        assert!(h.mobility_weight == 0.0, "Zenith should have zero mobility");
        assert!(h.piece_values[16] >= 18.0, "Zenith Queen should be very high");
    }

    #[test]
    fn test_all_different() {
        let omega = omega_heuristics();
        let apex = apex_heuristics();
        let zenith = zenith_heuristics();

        assert!(omega.center_weight != apex.center_weight);
        assert!(apex.center_weight != zenith.center_weight);
    }

    #[test]
    fn test_kings_zero() {
        for h in [omega_heuristics(), apex_heuristics(), zenith_heuristics()] {
            for i in 25..30 {
                assert_eq!(h.piece_values[i], 0.0);
            }
        }
    }
}
