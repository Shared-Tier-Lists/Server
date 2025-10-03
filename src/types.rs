use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectContents {
    tier_rows_html: String,
    image_carousel_html: String,
}
