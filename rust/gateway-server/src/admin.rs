use gateway_protocol::{
    DeviceListResult, DeviceRevokeResult, HostDiagnosticsResult, HostLifecycleResult,
    HostStatusResult, PairingCreateResult, PairingListResult, ProtocolError,
};

pub trait HostAdmin: Send + Sync {
    fn status(&self) -> HostStatusResult;
    fn diagnostics(&self) -> HostDiagnosticsResult;
    fn request_start(&self) -> Result<HostLifecycleResult, ProtocolError>;
    fn request_stop(&self) -> Result<HostLifecycleResult, ProtocolError>;
    fn request_restart(&self) -> Result<HostLifecycleResult, ProtocolError>;
    fn begin_pairing(&self, label: &str) -> Result<PairingCreateResult, ProtocolError>;
    fn list_pairings(&self) -> Result<PairingListResult, ProtocolError>;
    fn list_devices(&self) -> Result<DeviceListResult, ProtocolError>;
    fn revoke_device(&self, device_id: &str) -> Result<DeviceRevokeResult, ProtocolError>;
}
