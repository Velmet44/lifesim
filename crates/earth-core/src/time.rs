#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    Hadean,
    Archean,
    Proterozoic,
    Phanerozoic,
    Quaternary,
    Twin,
}

impl Era {
    pub fn of_year(t_yr: f64) -> Self {
        if t_yr < -4_000_000_000.0 {
            Era::Hadean
        } else if t_yr < -2_500_000_000.0 {
            Era::Archean
        } else if t_yr < -541_000_000.0 {
            Era::Proterozoic
        } else if t_yr < -2_580_000.0 {
            Era::Phanerozoic
        } else if t_yr < 0.0 {
            Era::Quaternary
        } else {
            Era::Twin
        }
    }
    /// Full DAG-pass interval (slow handshake) per docs/era-dt.md.
    pub fn sync_yr(&self) -> f64 {
        match self {
            Era::Hadean => 1e6,
            Era::Archean => 1e5,
            Era::Proterozoic => 1e4,
            Era::Phanerozoic => 1e3,
            Era::Quaternary => 1.0,
            Era::Twin => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SimTime {
    pub t_yr: f64,
}

impl SimTime {
    pub fn new(t_yr: f64) -> Self {
        Self { t_yr }
    }
    pub fn era(&self) -> Era {
        Era::of_year(self.t_yr)
    }
}
