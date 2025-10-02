use mongodb::bson::Document;
use crate::error;

pub fn get_string_field(
    doc: &Document,
    field_name: &str
) -> error::Result<String> {
    Ok(doc.get_str(field_name)?.to_string())
}
