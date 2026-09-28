pub mod coordinates;
pub mod error;
pub mod events;
pub mod ownership;
pub mod state;
pub mod topology;

pub use coordinates::{DisplayBounds, NormalizedPoint};
pub use error::{CoreError, Result};
pub use events::{ElementState, InputEvent, KeyCode, KeyboardEvent, MouseButton, MouseEvent};
pub use ownership::{ActiveDevice, DeviceOwnershipState, ExclusiveOwnershipRouter, RoutingDecision};
pub use state::{CursorMemory, HeldInputState};
pub use topology::{CircularTopology, Edge, InputRouter, ScreenLayout, ScreenTopology, TransferEvent};
