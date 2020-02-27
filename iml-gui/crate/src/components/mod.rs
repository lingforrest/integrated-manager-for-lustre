pub mod dropdown;
pub mod lnet_status;
pub mod modal;
pub mod paging;
pub mod popover;
pub mod table;
pub mod tooltip;

pub(crate) mod action_dropdown;
pub(crate) mod activity_indicator;
pub(crate) mod alert_indicator;
pub(crate) mod arrow;
pub(crate) mod attrs;
pub(crate) mod breadcrumbs;
pub(crate) mod charts;
pub(crate) mod font_awesome;
pub(crate) mod grafana_chart;
pub(crate) mod loading;
pub(crate) mod lock_indicator;
pub(crate) mod logo;
pub(crate) mod pie_chart;
pub(crate) mod restrict;
pub(crate) mod stratagem;
pub(crate) mod tree;

pub(crate) use activity_indicator::{activity_indicator, update_activity_health, ActivityHealth};
pub(crate) use alert_indicator::alert_indicator;
pub(crate) use arrow::arrow;
pub(crate) use font_awesome::{font_awesome, font_awesome_outline};
pub use logo::{ddn_logo, whamcloud_logo};
pub(crate) use pie_chart::pie_chart;

#[derive(Debug, Clone, Copy)]
pub enum Placement {
    Left,
    Right,
    Top,
    Bottom,
}

impl From<&Placement> for &str {
    fn from(p: &Placement) -> Self {
        match p {
            Placement::Left => "left",
            Placement::Right => "right",
            Placement::Top => "top",
            Placement::Bottom => "bottom",
        }
    }
}
