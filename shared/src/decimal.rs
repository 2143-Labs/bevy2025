use serde::{Deserialize, Serialize};

/// Whole parts + fractional (1/1 millionth)
/// first part is whole number (u64)
/// second part is fractional (.000_000 to .999999) (u16)
/// third part is negative flag
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Decimal(u64, u16, bool);

impl Decimal {
    pub fn new(f: f64) -> Self {
        let negative = f < 0.0;
        let abs_f = f.abs();
        let whole = abs_f.trunc() as u64;
        let fractional = ((abs_f.fract()) * 1_000_000.0).round() as u16;
        Decimal(whole, fractional, negative)
    }

    pub fn to_f64(&self) -> f64 {
        let value = self.0 as f64 + (self.1 as f64 / 1_000_000.0);
        if self.2 {
            -value
        } else {
            value
        }
    }

    pub fn to_f32(&self) -> f32 {
        self.to_f64() as f32
    }
}

impl std::fmt::Display for Decimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.2 {
            write!(f, "-{}.{:06}", self.0, self.1)
        } else {
            write!(f, "{}.{:06}", self.0, self.1)
        }
    }
}

impl std::ops::Add for Decimal {
    type Output = Decimal;

    fn add(self, other: Decimal) -> Decimal {
        let sum = self.to_f64() + other.to_f64();
        Decimal::new(sum)
    }
}

impl std::ops::Sub for Decimal {
    type Output = Decimal;

    fn sub(self, other: Decimal) -> Decimal {
        let diff = self.to_f64() - other.to_f64();
        Decimal::new(diff)
    }
}

impl std::ops::Mul for Decimal {
    type Output = Decimal;

    fn mul(self, other: Decimal) -> Decimal {
        let prod = self.to_f64() * other.to_f64();
        Decimal::new(prod)
    }
}

impl std::ops::Div for Decimal {
    type Output = Decimal;

    fn div(self, other: Decimal) -> Decimal {
        let quot = self.to_f64() / other.to_f64();
        Decimal::new(quot)
    }
}

impl std::ops::Neg for Decimal {
    type Output = Decimal;

    fn neg(self) -> Decimal {
        Decimal(self.0, self.1, !self.2)
    }
}

impl std::ops::AddAssign for Decimal {
    fn add_assign(&mut self, other: Decimal) {
        *self = *self + other;
    }
}
impl std::ops::SubAssign for Decimal {
    fn sub_assign(&mut self, other: Decimal) {
        *self = *self - other;
    }
}
impl std::ops::MulAssign for Decimal {
    fn mul_assign(&mut self, other: Decimal) {
        *self = *self * other;
    }
}
impl std::ops::DivAssign for Decimal {
    fn div_assign(&mut self, other: Decimal) {
        *self = *self / other;
    }
}


impl From<f64> for Decimal {
    fn from(f: f64) -> Self {
        Decimal::new(f)
    }
}

