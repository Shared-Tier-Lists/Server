use mongodb::bson::oid::ObjectId;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ClientMessage {
    OpenProject {
        project_id: ObjectId
    },
    EditProject {
        tier_container_html: String,
        image_carousel_html: String,
    }
}


#[derive(Debug, Deserialize, Clone)]
pub struct ProjectContentsResponse {
    pub(crate) tier_container_html: String,
    pub(crate) image_carousel_html: String,
}
