//! Game release regions.
//!
//! Each region corresponds to a distinct disc/save serial and may use a
//! different game key and, potentially, a different save layout. Europe
//! (`UCES00995`) is the first supported region; others are reserved.

/// A supported Patapon release region, identified by its save serial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// Europe, `UCES00995`.
    Europe,
    /// North America, `UCUS98711`.
    NorthAmerica,
    /// Japan, `UCJS10077`.
    Japan,
}

impl Region {
    /// Returns the region whose save directory uses `serial` (for example
    /// `"UCES00995"`), or `None` if the serial is not recognised.
    pub fn from_serial(serial: &str) -> Option<Self> {
        match serial {
            "UCES00995" => Some(Region::Europe),
            "UCUS98711" => Some(Region::NorthAmerica),
            "UCJS10077" => Some(Region::Japan),
            _ => None,
        }
    }

    /// Detects the region from a save directory name (for example
    /// `"UCES00995_DATA01"`), matching the leading serial. Returns `None` if
    /// no known serial is a prefix.
    pub fn detect(dir_name: &str) -> Option<Self> {
        [Region::Europe, Region::NorthAmerica, Region::Japan]
            .into_iter()
            .find(|r| dir_name.starts_with(r.serial()))
    }

    /// The canonical save serial for this region.
    pub fn serial(self) -> &'static str {
        match self {
            Region::Europe => "UCES00995",
            Region::NorthAmerica => "UCUS98711",
            Region::Japan => "UCJS10077",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_round_trips() {
        for region in [Region::Europe, Region::NorthAmerica, Region::Japan] {
            assert_eq!(Region::from_serial(region.serial()), Some(region));
        }
    }

    #[test]
    fn unknown_serial_is_none() {
        assert_eq!(Region::from_serial("UCES00000"), None);
    }
}
