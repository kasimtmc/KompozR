//! Constraints, the sizes a layout may take, and Velocity (Compose's `ui-unit` Constraints and Velocity). Constraints
//! live in kompoz-graphics (as Compose's ui-unit, which text lays out in too) and are re-exported here.

pub use kompoz_graphics::Constraints;
use std::fmt;

/// How fast something moves, in px per second along each axis (Compose's `ui-unit` Velocity): what a velocity
/// tracker measures, and what flings and nested scrolling hand on.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl Velocity {
    pub const Zero: Velocity = Velocity { x: 0.0, y: 0.0 };
    pub const fn new(x: f32, y: f32) -> Velocity {
        Velocity { x, y }
    }
    /// A copy with an axis replaced (Compose's `copy`).
    pub fn copy(self, x: Option<f32>, y: Option<f32>) -> Velocity {
        Velocity { x: x.unwrap_or(self.x), y: y.unwrap_or(self.y) }
    }
}

impl std::ops::Add for Velocity {
    type Output = Velocity;
    fn add(self, o: Velocity) -> Velocity {
        Velocity::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Velocity {
    type Output = Velocity;
    fn sub(self, o: Velocity) -> Velocity {
        Velocity::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Neg for Velocity {
    type Output = Velocity;
    fn neg(self) -> Velocity {
        Velocity::new(-self.x, -self.y)
    }
}

impl std::ops::Mul<f32> for Velocity {
    type Output = Velocity;
    fn mul(self, k: f32) -> Velocity {
        Velocity::new(self.x * k, self.y * k)
    }
}

impl std::ops::Div<f32> for Velocity {
    type Output = Velocity;
    fn div(self, k: f32) -> Velocity {
        Velocity::new(self.x / k, self.y / k)
    }
}

impl fmt::Debug for Velocity {
    /// Compose's toString.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({:.2}, {:.2}) px/sec", self.x, self.y)
    }
}
