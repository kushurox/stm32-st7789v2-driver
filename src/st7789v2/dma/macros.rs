
#[macro_export]
macro_rules! cs_command {
    ($self:expr, $cmd:expr, $delay_ms:expr) => {{
        $self.cs.set_low().ok(); // Select device
        $self.send_command($cmd); // Send command (CS stays low)
        $self.d.delay_ms($delay_ms); // Delay while CS is still low for processing
        $self.cs.set_high().ok(); // Deselect device after delay
    }};
}

// Macro for handling CS timing with data
#[macro_export]
macro_rules! cs_data {
    ($self:expr, $data:expr, $delay_ms:expr) => {{
        $self.cs.set_low().ok(); // Select device
        $self.send_data_u8($data); // Send data (CS stays low)
        $self.d.delay_ms($delay_ms); // Delay while CS is still low for processing
        $self.cs.set_high().ok(); // Deselect device after delay
    }};
}

// Macro for command+data sequence with proper CS timing
#[macro_export]
macro_rules! cs_command_data_sequence {
    ($self:expr, $cmd:expr, $data_method:ident, $cmd_delay:expr, $data_delay:expr) => {{
        $self.cs.set_low().ok(); // Select device for entire sequence
        $self.send_command($cmd); // Send command (CS stays low)
        $self.d.delay_ms($cmd_delay); // Command processing delay
        $self.$data_method($data_delay); // Send data (CS stays low)
        $self.cs.set_high().ok(); // Deselect device after entire sequence
    }};
}