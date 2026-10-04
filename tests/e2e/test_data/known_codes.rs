// Erya 1.0 has no backend API or enum payload. Keep this explicit so a future
// network-backed feature must extend the health guard instead of bypassing it.
pub const KNOWN_BACKEND_CODES: &[&str] = &[];
