//! Start-up settings read from the environment. Parsing is pure so it can be unit-tested; the
//! `from_env` wrappers are the only part that touches `std::env`.

/// Where a run starts when `RNS_START` is unset: just outside Goldshire in Elwynn Forest, in raw
/// WoW world coordinates (`x` north, `y` west). Small, open, gently sloped ground.
pub const DEFAULT_START: (f32, f32) = (-9464.0, 62.0);

/// The height (WoW `z`) the hero waits at until the terrain under it has streamed in, when the
/// start point names none. The hero snaps to the ground as soon as the heightfield is resident.
pub const DEFAULT_START_Z: f32 = 100.0;

/// The env var naming the start point: `x,y` or `x,y,z`, raw WoW world coordinates.
pub const START_VAR: &str = "RNS_START";

/// A start point in WoW world coordinates. `z` is `None` when it should come from the ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StartPoint {
    pub x: f32,
    pub y: f32,
    pub z: Option<f32>,
}

impl Default for StartPoint {
    fn default() -> Self {
        Self {
            x: DEFAULT_START.0,
            y: DEFAULT_START.1,
            z: None,
        }
    }
}

impl StartPoint {
    /// The point as a WoW `[x, y, z]`, with [`DEFAULT_START_Z`] standing in for a missing `z`.
    pub fn wow(&self) -> [f32; 3] {
        [self.x, self.y, self.z.unwrap_or(DEFAULT_START_Z)]
    }

    /// Reads [`START_VAR`]; absent or malformed falls back to the default (with a warning).
    pub fn from_env() -> Self {
        match std::env::var(START_VAR) {
            Ok(v) => parse_start(&v).unwrap_or_else(|| {
                eprintln!(
                    "rnsurvivors: {START_VAR}={v:?} wants `x,y` or `x,y,z` — using the default"
                );
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

/// Parses `x,y` or `x,y,z` (whitespace allowed, all finite numbers). Anything else is `None`.
pub fn parse_start(s: &str) -> Option<StartPoint> {
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    let nums: Option<Vec<f32>> = parts
        .iter()
        .map(|p| p.parse::<f32>().ok().filter(|v| v.is_finite()))
        .collect();
    match nums?.as_slice() {
        [x, y] => Some(StartPoint {
            x: *x,
            y: *y,
            z: None,
        }),
        [x, y, z] => Some(StartPoint {
            x: *x,
            y: *y,
            z: Some(*z),
        }),
        _ => None,
    }
}

/// Parses a positive finite float, for the speed override.
pub fn parse_positive(s: &str) -> Option<f32> {
    s.trim()
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_components_leave_z_to_the_ground() {
        assert_eq!(
            parse_start("-9464, 62"),
            Some(StartPoint {
                x: -9464.0,
                y: 62.0,
                z: None
            })
        );
    }

    #[test]
    fn three_components_pin_z() {
        let p = parse_start("1.5,-2,3").unwrap();
        assert_eq!(p.z, Some(3.0));
        assert_eq!(p.wow(), [1.5, -2.0, 3.0]);
    }

    #[test]
    fn malformed_is_rejected() {
        assert_eq!(parse_start(""), None);
        assert_eq!(parse_start("1"), None);
        assert_eq!(parse_start("1,2,3,4"), None);
        assert_eq!(parse_start("a,b"), None);
        assert_eq!(parse_start("1,,2"), None);
        assert_eq!(parse_start("NaN,1"), None);
        assert_eq!(parse_start("inf,1"), None);
    }

    #[test]
    fn default_z_stands_in() {
        assert_eq!(StartPoint::default().wow()[2], DEFAULT_START_Z);
    }

    #[test]
    fn positive_only() {
        assert_eq!(parse_positive(" 7.5 "), Some(7.5));
        assert_eq!(parse_positive("0"), None);
        assert_eq!(parse_positive("-3"), None);
        assert_eq!(parse_positive("x"), None);
    }
}
