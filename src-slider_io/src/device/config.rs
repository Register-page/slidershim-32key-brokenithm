use serde_json::Value;

#[derive(Debug, Clone)]
pub enum HardwareSpec {
  TasollerOne,
  TasollerTwo,
  Yuancon,
  YuanconThree,
  Yubideck,
  YubideckThree,
}

#[derive(Debug, Clone)]
pub enum BrokenithmSpec {
  Basic,
  GroundOnly,
  Nostalgia,
}

#[derive(Debug, Clone)]
pub enum DeviceMode {
  None,
  Hardware {
    spec: HardwareSpec,
    disable_air: bool,
  },
  Brokenithm {
    spec: BrokenithmSpec,
    lights_enabled: bool,
    port: u16,
    ground_percent: u8,
  },
  DivaSlider {
    port: String,
    brightness: u8,
  },
}

impl DeviceMode {
  pub fn from_serde_value(v: &Value) -> Option<Self> {
    Some(match v["deviceMode"].as_str()? {
      "none" => DeviceMode::None,
      "tasoller-one" => DeviceMode::Hardware {
        spec: HardwareSpec::TasollerOne,
        disable_air: v["disableAirStrings"].as_bool()?,
      },
      "tasoller-two" => DeviceMode::Hardware {
        spec: HardwareSpec::TasollerTwo,
        disable_air: v["disableAirStrings"].as_bool()?,
      },
      "yuancon" => DeviceMode::Hardware {
        spec: HardwareSpec::Yuancon,
        disable_air: v["disableAirStrings"].as_bool()?,
      },
      "yuancon-three" => DeviceMode::Hardware {
        spec: HardwareSpec::YuanconThree,
        disable_air: v["disableAirStrings"].as_bool()?,
      },
      "yubideck" => DeviceMode::Hardware {
        spec: HardwareSpec::Yubideck,
        disable_air: v["disableAirStrings"].as_bool()?,
      },
      "yubideck-three" => DeviceMode::Hardware {
        spec: HardwareSpec::YubideckThree,
        disable_air: v["disableAirStrings"].as_bool()?,
      },
      "diva" => DeviceMode::DivaSlider {
        port: v["divaSerialPort"].as_str()?.to_string(),
        brightness: u8::try_from(v["divaBrightness"].as_i64()?).ok()?,
      },
      "brokenithm" => DeviceMode::Brokenithm {
        spec: match v["disableAirStrings"].as_bool()? {
          false => BrokenithmSpec::Basic,
          true => BrokenithmSpec::GroundOnly,
        },
        lights_enabled: false,
        port: u16::try_from(v["brokenithmPort"].as_i64()?)
          .ok()
          .or(Some(1606))?,
        ground_percent: brokenithm_ground_percent(v),
      },
      "brokenithm-led" => DeviceMode::Brokenithm {
        spec: match v["disableAirStrings"].as_bool()? {
          false => BrokenithmSpec::Basic,
          true => BrokenithmSpec::GroundOnly,
        },
        lights_enabled: true,
        port: u16::try_from(v["brokenithmPort"].as_i64()?)
          .ok()
          .or(Some(1606))?,
        ground_percent: brokenithm_ground_percent(v),
      },
      "brokenithm-nostalgia" => DeviceMode::Brokenithm {
        spec: BrokenithmSpec::Nostalgia,
        lights_enabled: false,
        port: u16::try_from(v["brokenithmPort"].as_i64()?)
          .ok()
          .or(Some(1606))?,
        ground_percent: brokenithm_ground_percent(v),
      },
      _ => return None,
    })
  }

  pub fn get_port(&self) -> Option<u16> {
    match self {
      DeviceMode::Brokenithm { port, .. } => Some(*port),
      _ => None,
    }
  }
}

fn brokenithm_ground_percent(v: &Value) -> u8 {
  v["brokenithmGroundPercent"]
    .as_u64()
    .and_then(|value| u8::try_from(value).ok())
    .unwrap_or(50)
    .clamp(20, 80)
}

#[cfg(test)]
mod tests {
  use super::{brokenithm_ground_percent, DeviceMode};
  use serde_json::json;

  #[test]
  fn old_brokenithm_configs_default_to_equal_areas() {
    let value = json!({
      "deviceMode": "brokenithm",
      "disableAirStrings": false,
      "brokenithmPort": 1606
    });

    assert_eq!(brokenithm_ground_percent(&value), 50);
    assert!(matches!(
      DeviceMode::from_serde_value(&value),
      Some(DeviceMode::Brokenithm {
        ground_percent: 50,
        ..
      })
    ));
  }

  #[test]
  fn ipad_ground_area_is_clamped() {
    assert_eq!(
      brokenithm_ground_percent(&json!({ "brokenithmGroundPercent": 5 })),
      20
    );
    assert_eq!(
      brokenithm_ground_percent(&json!({ "brokenithmGroundPercent": 95 })),
      80
    );
    assert_eq!(
      brokenithm_ground_percent(&json!({ "brokenithmGroundPercent": 65 })),
      65
    );
  }
}
