mod colors;
mod graph_options;
mod options;
mod render;

pub use colors::{COLOR_ROLES, ColorError, ColorScheme, Rgb};
pub use graph_options::{Background, GraphColors, GraphMetric, GraphOptions, GraphRange};
pub use options::{CardKind, CardOptions, CardSize, CardTheme, PeriodDays, PeriodDaysError};
pub use render::{
    CardRenderError, CardRenderer, GraphError, SvgCardRenderer, render, render_contributors,
    render_graph, render_graph_error, render_languages, render_overview,
};
