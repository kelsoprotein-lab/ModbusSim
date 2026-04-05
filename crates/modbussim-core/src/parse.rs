use crate::register::{RegisterType, DataType, Endian};
use crate::master::ReadFunction;

pub fn parse_register_type(s: &str) -> Result<RegisterType, String> {
    match s {
        "coil" => Ok(RegisterType::Coil),
        "discrete_input" => Ok(RegisterType::DiscreteInput),
        "input_register" => Ok(RegisterType::InputRegister),
        "holding_register" => Ok(RegisterType::HoldingRegister),
        _ => Err(format!("unknown register type: {}", s)),
    }
}

pub fn parse_endian(s: &str) -> Result<Endian, String> {
    match s {
        "big" => Ok(Endian::Big),
        "little" => Ok(Endian::Little),
        "mid_big" => Ok(Endian::MidBig),
        "mid_little" => Ok(Endian::MidLittle),
        _ => Err(format!("unknown endian: {}", s)),
    }
}

pub fn parse_data_type(s: &str) -> Result<DataType, String> {
    match s {
        "bool" => Ok(DataType::Bool),
        "uint16" => Ok(DataType::UInt16),
        "int16" => Ok(DataType::Int16),
        "uint32" => Ok(DataType::UInt32),
        "int32" => Ok(DataType::Int32),
        "float32" => Ok(DataType::Float32),
        _ => Err(format!("unknown data type: {}", s)),
    }
}

pub fn parse_read_function(s: &str) -> Result<ReadFunction, String> {
    match s {
        "read_coils" => Ok(ReadFunction::ReadCoils),
        "read_discrete_inputs" => Ok(ReadFunction::ReadDiscreteInputs),
        "read_holding_registers" => Ok(ReadFunction::ReadHoldingRegisters),
        "read_input_registers" => Ok(ReadFunction::ReadInputRegisters),
        _ => Err(format!("unknown function: {}", s)),
    }
}

pub fn read_function_to_string(f: ReadFunction) -> &'static str {
    match f {
        ReadFunction::ReadCoils => "read_coils",
        ReadFunction::ReadDiscreteInputs => "read_discrete_inputs",
        ReadFunction::ReadHoldingRegisters => "read_holding_registers",
        ReadFunction::ReadInputRegisters => "read_input_registers",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_register_type_valid() {
        assert_eq!(parse_register_type("coil").unwrap(), RegisterType::Coil);
        assert_eq!(parse_register_type("discrete_input").unwrap(), RegisterType::DiscreteInput);
        assert_eq!(parse_register_type("input_register").unwrap(), RegisterType::InputRegister);
        assert_eq!(parse_register_type("holding_register").unwrap(), RegisterType::HoldingRegister);
    }

    #[test]
    fn test_parse_register_type_invalid() {
        assert!(parse_register_type("coils").is_err());
        assert!(parse_register_type("").is_err());
        assert!(parse_register_type("Coil").is_err());
        let err = parse_register_type("bad").unwrap_err();
        assert!(err.contains("bad"));
    }

    #[test]
    fn test_parse_endian_valid() {
        assert_eq!(parse_endian("big").unwrap(), Endian::Big);
        assert_eq!(parse_endian("little").unwrap(), Endian::Little);
        assert_eq!(parse_endian("mid_big").unwrap(), Endian::MidBig);
        assert_eq!(parse_endian("mid_little").unwrap(), Endian::MidLittle);
    }

    #[test]
    fn test_parse_endian_invalid() {
        assert!(parse_endian("Big").is_err());
        assert!(parse_endian("").is_err());
        assert!(parse_endian("be").is_err());
        let err = parse_endian("unknown").unwrap_err();
        assert!(err.contains("unknown"));
    }

    #[test]
    fn test_parse_data_type_valid() {
        assert_eq!(parse_data_type("bool").unwrap(), DataType::Bool);
        assert_eq!(parse_data_type("uint16").unwrap(), DataType::UInt16);
        assert_eq!(parse_data_type("int16").unwrap(), DataType::Int16);
        assert_eq!(parse_data_type("uint32").unwrap(), DataType::UInt32);
        assert_eq!(parse_data_type("int32").unwrap(), DataType::Int32);
        assert_eq!(parse_data_type("float32").unwrap(), DataType::Float32);
    }

    #[test]
    fn test_parse_data_type_invalid() {
        assert!(parse_data_type("float64").is_err());
        assert!(parse_data_type("").is_err());
        assert!(parse_data_type("UInt16").is_err());
        let err = parse_data_type("bad_type").unwrap_err();
        assert!(err.contains("bad_type"));
    }

    #[test]
    fn test_parse_read_function_valid() {
        assert_eq!(parse_read_function("read_coils").unwrap(), ReadFunction::ReadCoils);
        assert_eq!(parse_read_function("read_discrete_inputs").unwrap(), ReadFunction::ReadDiscreteInputs);
        assert_eq!(parse_read_function("read_holding_registers").unwrap(), ReadFunction::ReadHoldingRegisters);
        assert_eq!(parse_read_function("read_input_registers").unwrap(), ReadFunction::ReadInputRegisters);
    }

    #[test]
    fn test_parse_read_function_invalid() {
        assert!(parse_read_function("ReadCoils").is_err());
        assert!(parse_read_function("").is_err());
        assert!(parse_read_function("fc01").is_err());
        let err = parse_read_function("bad_fn").unwrap_err();
        assert!(err.contains("bad_fn"));
    }

    #[test]
    fn test_read_function_to_string_all_variants() {
        assert_eq!(read_function_to_string(ReadFunction::ReadCoils), "read_coils");
        assert_eq!(read_function_to_string(ReadFunction::ReadDiscreteInputs), "read_discrete_inputs");
        assert_eq!(read_function_to_string(ReadFunction::ReadHoldingRegisters), "read_holding_registers");
        assert_eq!(read_function_to_string(ReadFunction::ReadInputRegisters), "read_input_registers");
    }

    #[test]
    fn test_read_function_round_trip() {
        let variants = [
            ReadFunction::ReadCoils,
            ReadFunction::ReadDiscreteInputs,
            ReadFunction::ReadHoldingRegisters,
            ReadFunction::ReadInputRegisters,
        ];
        for v in variants {
            let s = read_function_to_string(v);
            assert_eq!(parse_read_function(s).unwrap(), v);
        }
    }
}
