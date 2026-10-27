//! Ctl argv and client-value adapter for the neutral settings file form.
use super::{
    Limits,
    call::{Call, Value, number, value},
    settings_adapter,
};
use alloc::boxed::Box;
use jig_client::contract as c;
use skein_lib::List;
pub(crate) fn selector(call: &Call) -> c::Setting {
    if call.form == 1 {
        return c::Setting::Rules;
    }
    let number =
        c::SettingNumber { number: u32::try_from(number(call, 1).expect("setting number")).expect("parsed u32") };
    match value(call, 0) {
        Some(Value::Word(0)) => c::Setting::Charter(number),
        Some(Value::Word(1)) => c::Setting::Template(number),
        Some(Value::Word(2)) => c::Setting::Policy(number),
        Some(_) | None => unreachable!("parsed setting kind"),
    }
}
pub(crate) fn request(call: &Call, limits: &Limits) -> Result<c::ChangeSettings, Box<[u8]>> {
    let Some(Value::Text(bytes)) = value(call, 0) else {
        return Err(Box::from(b"settings file: FILE: input not loaded".as_slice()));
    };
    Ok(c::ChangeSettings {
        against: u32::try_from(
            number(call, if call.tool == crate::jig::rows::PROPOSE_SETTINGS_AT { 2 } else { 1 }).expect("against"),
        )
        .expect("parsed u32"),
        items: decode(bytes, limits)?,
        running: match value(call, 2) {
            Some(Value::Word(1)) => c::Running::Moves,
            Some(_) | None => c::Running::Stays,
        },
    })
}

/// Decode the settings file into client items using the shared neutral form.
pub fn decode(bytes: &[u8], limits: &Limits) -> Result<List<c::SettingItem>, Box<[u8]>> {
    let items = settings_form::file::decode(bytes, &settings_adapter::limits(limits))?;
    let mut out = List::with_capacity(items.len());
    for item in &items {
        out.push(settings_adapter::client_setting_item(item)).expect("one client item per form item");
    }
    Ok(out)
}
