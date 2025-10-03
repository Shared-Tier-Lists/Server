use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectContents {
    pub tier_rows_html: String,
    pub image_carousel_html: String,
}
