use std::ops;

use super::object_attributes::RotationScaling;

/// A simple struct to represent a point in a carthesian plane.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(super) struct Point<T> {
    pub(super) x: T,
    pub(super) y: T,
}

impl<T> Point<T> {
    pub(super) const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    pub(super) fn map<U>(self, f: fn(T) -> U) -> Point<U> {
        Point::<U> {
            x: f(self.x),
            y: f(self.y),
        }
    }
}

impl<T> ops::Add<Self> for Point<T>
where
    T: ops::Add<Output = T>,
{
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T> ops::Sub<Self> for Point<T>
where
    T: ops::Sub<Output = T>,
{
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl ops::Mul<RotationScaling> for Point<f64> {
    type Output = Self;
    fn mul(self, rhs: RotationScaling) -> Self::Output {
        let r = rhs.apply(self.x, self.y);

        Self { x: r.0, y: r.1 }
    }
}

impl<T> ops::Mul<T> for Point<T>
where
    T: ops::Mul<Output = T> + Copy,
{
    type Output = Self;
    fn mul(self, rhs: T) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl<T> ops::Div<T> for Point<T>
where
    T: ops::Div<Output = T> + Copy,
{
    type Output = Self;
    fn div(self, rhs: T) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Point;

    #[test]
    fn test_point() {
        let p = Point { x: 10_u16, y: 10 };

        assert_eq!(p / 2, Point { x: 5_u16, y: 5 });
        assert_eq!(p * 2, Point { x: 20_u16, y: 20 });
        assert_eq!(p + Point { x: 1_u16, y: 1 }, Point { x: 11_u16, y: 11 });
        assert_eq!(p - Point { x: 1_u16, y: 1 }, Point { x: 9_u16, y: 9 });
    }
}
