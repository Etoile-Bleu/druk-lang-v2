use std::fmt;
use std::str::FromStr;

use inkwell::OptimizationLevel;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OptLevel {
    O0,
    O1,
    #[default]
    O2,
    O3,
}

impl OptLevel {
    pub(crate) fn to_inkwell(self) -> OptimizationLevel {
        match self {
            OptLevel::O0 => OptimizationLevel::None,
            OptLevel::O1 => OptimizationLevel::Less,
            OptLevel::O2 => OptimizationLevel::Default,
            OptLevel::O3 => OptimizationLevel::Aggressive,
        }
    }
}

impl fmt::Display for OptLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digit = match self {
            OptLevel::O0 => '0',
            OptLevel::O1 => '1',
            OptLevel::O2 => '2',
            OptLevel::O3 => '3',
        };
        write!(f, "{digit}")
    }
}

impl FromStr for OptLevel {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "0" | "O0" => Ok(OptLevel::O0),
            "1" | "O1" => Ok(OptLevel::O1),
            "2" | "O2" => Ok(OptLevel::O2),
            "3" | "O3" => Ok(OptLevel::O3),
            other => Err(format!(
                "'{other}' is not an optimization level; use 0, 1, 2 or 3"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_digits_and_o_prefixed_forms() {
        for (text, expected) in [
            ("0", OptLevel::O0),
            ("O1", OptLevel::O1),
            ("2", OptLevel::O2),
            ("O3", OptLevel::O3),
        ] {
            assert_eq!(text.parse::<OptLevel>(), Ok(expected));
        }
    }

    #[test]
    fn rejects_unknown_levels_with_a_plain_message() {
        let message = "4".parse::<OptLevel>().expect_err("4 is not a level");
        assert_eq!(
            message,
            "'4' is not an optimization level; use 0, 1, 2 or 3"
        );
        assert!("s".parse::<OptLevel>().is_err());
        assert!("".parse::<OptLevel>().is_err());
    }

    #[test]
    fn maps_each_level_to_a_distinct_inkwell_level() {
        assert_eq!(OptLevel::O0.to_inkwell(), OptimizationLevel::None);
        assert_eq!(OptLevel::O1.to_inkwell(), OptimizationLevel::Less);
        assert_eq!(OptLevel::O2.to_inkwell(), OptimizationLevel::Default);
        assert_eq!(OptLevel::O3.to_inkwell(), OptimizationLevel::Aggressive);
    }

    #[test]
    fn display_round_trips_through_parse() {
        for level in [OptLevel::O0, OptLevel::O1, OptLevel::O2, OptLevel::O3] {
            assert_eq!(level.to_string().parse::<OptLevel>(), Ok(level));
        }
    }
}
