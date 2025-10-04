pub enum Collections {}
impl Collections {
    pub const USERS: &'static str = "users";
    pub const PROJECTS: &'static str = "projects";
}

pub enum UserFields {}
impl UserFields {
    pub const ID: &'static str = "_id";
    pub const EMAIL: &'static str = "email";
    pub const DISPLAY_NAME: &'static str = "display_name";
    pub const PASSWD_HASH: &'static str = "passwd_hash";
    pub const PROJECTS: &'static str = "projects";
}

pub enum ProjectFields {}
impl ProjectFields {
    pub const ID: &'static str = "_id";
    pub const NAME: &'static str = "name";
    pub const TEMPLATE_LINK: &'static str = "template_link";
    pub const CONTRIBUTORS: &'static str = "contributors";
    pub const TIER_CONTAINER_HTML: &'static str = "tier_container_html";
    pub const IMAGE_CAROUSEL_HTML: &'static str = "image_carousel_html";
}
