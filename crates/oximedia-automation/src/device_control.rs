//! High-level broadcast device control automation.
//!
//! Provides types for tracking broadcast devices and issuing commands to them.
//! Note: The lower-level protocol-specific controllers live in `device::control`.

/// Category of a broadcast device.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BroadcastDeviceType {
    /// Video router / matrix switcher.
    Router,
    /// Broadcast monitor.
    Monitor,
    /// Studio or remote camera.
    Camera,
    /// Video/audio recorder.
    Recorder,
    /// Production switcher.
    Switcher,
    /// Playout server.
    Playout,
}

impl BroadcastDeviceType {
    /// Return the typical control protocol name for the device type.
    pub fn typical_protocol(&self) -> &str {
        match self {
            Self::Router => "NVISION",
            Self::Monitor => "SNMP",
            Self::Camera => "VISCA",
            Self::Recorder => "Sony-9pin",
            Self::Switcher => "Ember+",
            Self::Playout => "VDCP",
        }
    }
}

/// A command sent to a broadcast device.
#[derive(Debug, Clone)]
pub struct BroadcastDeviceCommand {
    /// Identifier of the target device.
    pub device_id: String,
    /// Command verb (e.g. `"play"`, `"stop"`, `"take"`).
    pub command: String,
    /// Optional parameters associated with the command.
    pub params: Vec<String>,
    /// Millisecond timestamp when the command was issued.
    pub timestamp_ms: u64,
}

impl BroadcastDeviceCommand {
    /// Create a new device command.
    pub fn new(device_id: &str, command: &str, params: Vec<String>, timestamp_ms: u64) -> Self {
        Self {
            device_id: device_id.to_string(),
            command: command.to_string(),
            params,
            timestamp_ms,
        }
    }

    /// Return `true` if the command carries at least one parameter.
    pub fn has_params(&self) -> bool {
        !self.params.is_empty()
    }
}

/// Observed state of a broadcast device.
#[derive(Debug, Clone)]
pub struct BroadcastDeviceState {
    /// Whether the device is considered online.
    pub online: bool,
    /// Network address of the device.
    pub address: String,
    /// Last time (ms) the device was successfully contacted.
    pub last_seen_ms: u64,
    /// Consecutive error count since last successful contact.
    pub error_count: u32,
}

impl BroadcastDeviceState {
    /// Create a new device state record.
    pub fn new(address: &str, online: bool, last_seen_ms: u64) -> Self {
        Self {
            online,
            address: address.to_string(),
            last_seen_ms,
            error_count: 0,
        }
    }

    /// Return `true` if the device was last seen within `timeout_ms` of `now_ms`.
    pub fn is_reachable(&self, now_ms: u64, timeout_ms: u64) -> bool {
        self.online && now_ms.saturating_sub(self.last_seen_ms) <= timeout_ms
    }
}

/// Registry of broadcast devices and their command history.
#[derive(Debug, Default)]
pub struct BroadcastDeviceController {
    devices: Vec<(String, BroadcastDeviceType, BroadcastDeviceState)>,
    command_log: Vec<BroadcastDeviceCommand>,
}

impl BroadcastDeviceController {
    /// Create a new, empty controller.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a device with the controller.
    ///
    /// If a device with the same `id` already exists it is replaced.
    pub fn register(&mut self, id: &str, device_type: BroadcastDeviceType, address: &str) {
        self.devices.retain(|(did, _, _)| did != id);
        self.devices.push((
            id.to_string(),
            device_type,
            BroadcastDeviceState::new(address, true, 0),
        ));
    }

    /// Issue a command to the device identified by `id`.
    ///
    /// Returns `true` if the device was found, `false` otherwise.
    pub fn send_command(&mut self, id: &str, cmd: &str, params: Vec<String>) -> bool {
        if self.devices.iter().any(|(did, _, _)| did == id) {
            self.command_log
                .push(BroadcastDeviceCommand::new(id, cmd, params, 0));
            true
        } else {
            false
        }
    }

    /// Return references to all devices that are currently marked online.
    pub fn online_devices(&self) -> Vec<(&str, &BroadcastDeviceType)> {
        self.devices
            .iter()
            .filter(|(_, _, state)| state.online)
            .map(|(id, dt, _)| (id.as_str(), dt))
            .collect()
    }

    /// Return the command history for a given device.
    pub fn command_history(&self, device_id: &str) -> Vec<&BroadcastDeviceCommand> {
        self.command_log
            .iter()
            .filter(|c| c.device_id == device_id)
            .collect()
    }

    /// Mark a device offline (simulates a connectivity loss).
    pub fn mark_offline(&mut self, id: &str) {
        for (did, _, state) in &mut self.devices {
            if did == id {
                state.online = false;
            }
        }
    }

    /// Return the number of registered devices.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_type_typical_protocol_router() {
        assert_eq!(BroadcastDeviceType::Router.typical_protocol(), "NVISION");
    }

    #[test]
    fn test_device_type_typical_protocol_camera() {
        assert_eq!(BroadcastDeviceType::Camera.typical_protocol(), "VISCA");
    }

    #[test]
    fn test_device_type_typical_protocol_recorder() {
        assert_eq!(
            BroadcastDeviceType::Recorder.typical_protocol(),
            "Sony-9pin"
        );
    }

    #[test]
    fn test_device_type_typical_protocol_playout() {
        assert_eq!(BroadcastDeviceType::Playout.typical_protocol(), "VDCP");
    }

    #[test]
    fn test_command_has_params_true() {
        let cmd = BroadcastDeviceCommand::new("dev1", "cue", vec!["clip_42".to_string()], 0);
        assert!(cmd.has_params());
    }

    #[test]
    fn test_command_has_params_false() {
        let cmd = BroadcastDeviceCommand::new("dev1", "stop", vec![], 0);
        assert!(!cmd.has_params());
    }

    #[test]
    fn test_device_state_is_reachable_yes() {
        let state = BroadcastDeviceState::new("192.168.1.1", true, 900);
        assert!(state.is_reachable(1000, 200));
    }

    #[test]
    fn test_device_state_is_reachable_no_timeout() {
        let state = BroadcastDeviceState::new("192.168.1.1", true, 500);
        assert!(!state.is_reachable(1000, 400));
    }

    #[test]
    fn test_device_state_is_reachable_offline() {
        let state = BroadcastDeviceState::new("192.168.1.1", false, 999);
        assert!(!state.is_reachable(1000, 5000));
    }

    #[test]
    fn test_controller_register_and_count() {
        let mut ctrl = BroadcastDeviceController::new();
        ctrl.register("cam1", BroadcastDeviceType::Camera, "10.0.0.1");
        ctrl.register("sw1", BroadcastDeviceType::Switcher, "10.0.0.2");
        assert_eq!(ctrl.device_count(), 2);
    }

    #[test]
    fn test_controller_register_replaces_duplicate() {
        let mut ctrl = BroadcastDeviceController::new();
        ctrl.register("rec1", BroadcastDeviceType::Recorder, "10.0.0.10");
        ctrl.register("rec1", BroadcastDeviceType::Recorder, "10.0.0.11");
        assert_eq!(ctrl.device_count(), 1);
    }

    #[test]
    fn test_controller_send_command_known_device() {
        let mut ctrl = BroadcastDeviceController::new();
        ctrl.register("play1", BroadcastDeviceType::Playout, "10.0.0.5");
        let ok = ctrl.send_command("play1", "play", vec![]);
        assert!(ok);
        assert_eq!(ctrl.command_history("play1").len(), 1);
    }

    #[test]
    fn test_controller_send_command_unknown_device() {
        let mut ctrl = BroadcastDeviceController::new();
        let ok = ctrl.send_command("ghost", "play", vec![]);
        assert!(!ok);
    }

    #[test]
    fn test_controller_online_devices() {
        let mut ctrl = BroadcastDeviceController::new();
        ctrl.register("cam1", BroadcastDeviceType::Camera, "10.0.0.1");
        ctrl.register("mon1", BroadcastDeviceType::Monitor, "10.0.0.2");
        ctrl.mark_offline("mon1");
        let online = ctrl.online_devices();
        assert_eq!(online.len(), 1);
        assert_eq!(online[0].0, "cam1");
    }
}
