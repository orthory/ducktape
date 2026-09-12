//! This node's operator screen as a view on the kernel contract: status,
//! standing, peers, the log ring and the code registry, rendered from a
//! wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`node.props`: connected, dark, this
//! seat's admin standing and tier, the app's connection reading, the
//! daemon's workspace directory and the wall clock). The node's own facts,
//! its peers and its code registry are read here through the kernel's
//! `rpc.status` / `rpc.peers` / `rpc.query`, re-read on every `rpc.live` hit
//! for the `block` plane, and the log ring arrives through `rpc.stream` on
//! the node's own `logs` topic — the timeline is this view's state, not the
//! app's. Retuning the running node's tracing filter leaves as one
//! `rpc.admin` POST the kernel signs with the seated key; the clipboard is
//! the one intent left, because it is an OS door.
pub mod host;
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
    App,
    AppDark,
}
#[derive(Clone, Copy)]
struct Palette {
    colors: [::ducktape_view_guest::wire::Rgba; 128],
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NodeTab {
    Overview,
    Permissions,
    Activity,
    Modules,
}
#[allow(dead_code)]
pub struct NodeView {
    pub(crate) active_palette: AppTheme,
    pub(crate) node_data_dir: String,
    pub(crate) tier: String,
    pub(crate) admin: bool,
    pub(crate) status: String,
    pub(crate) wall_now: i64,
    pub(crate) connected: bool,
    pub(crate) connection_serial: i64,
    pub(crate) node_tab: NodeTab,
    pub(crate) facts: crate::host::NodeFacts,
    pub(crate) loading: bool,
    pub(crate) module_rows: Vec<crate::host::ModuleRow>,
    pub(crate) node_peers: Vec<crate::host::PeerRow>,
    pub(crate) log_lines: Vec<crate::host::LogRow>,
    pub(crate) node_log_filter: String,
    pub(crate) live_log_filter: String,
    pub(crate) live_filter_note: String,
    pub(crate) host_error: String,
    pub(crate) sent: bool,
}
impl ::std::fmt::Debug for NodeView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("NodeView")
    }
}
#[derive(Clone)]
pub enum Message {
    SessionArrived(crate::host::SessionItem),
    FactsArrived(crate::host::FactsItem),
    PeersArrived(crate::host::PeersItem),
    ModulesArrived(crate::host::ModulesItem),
    LogsArrived(crate::host::LogItem),
    ActDone(crate::host::ActItem),
    SelectNodeTab(NodeTab),
    OpenNodeModules,
    NodeLogFilterChanged(String),
    LiveLogFilterChanged(String),
    ApplyLiveLogFilter,
    CopyToClipboard(String, String),
    BindNodeLogFilter(String),
    BindLiveLogFilter(String),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl NodeView {
    fn palette(&self) -> Palette {
        match self.active_palette.clone() {
            AppTheme::App => {
                Palette {
                    colors: [
                        ::ducktape_view_guest::wire::Rgba([
                            58.0 / 255.0,
                            56.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            212.0 / 255.0,
                            210.0 / 255.0,
                            202.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            253.0 / 255.0,
                            251.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            44.0 / 255.0,
                            43.0 / 255.0,
                            39.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            107.0 / 255.0,
                            105.0 / 255.0,
                            98.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            246.0 / 255.0,
                            245.0 / 255.0,
                            242.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            50.0 / 255.0,
                            47.0 / 255.0,
                            40.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            235.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            179.0 / 255.0,
                            177.0 / 255.0,
                            168.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            94.0 / 255.0,
                            92.0 / 255.0,
                            85.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            243.0 / 255.0,
                            242.0 / 255.0,
                            239.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            63.0 / 255.0,
                            62.0 / 255.0,
                            57.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            160.0 / 255.0,
                            90.0 / 255.0,
                            60.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            249.0 / 255.0,
                            241.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            231.0 / 255.0,
                            210.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            184.0 / 255.0,
                            84.0 / 255.0,
                            76.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            244.0 / 255.0,
                            243.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            239.0 / 255.0,
                            214.0 / 255.0,
                            211.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            224.0 / 255.0,
                            101.0 / 255.0,
                            92.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            95.0 / 255.0,
                            158.0 / 255.0,
                            116.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            21.0 / 255.0,
                            20.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            238.0 / 255.0,
                            245.0 / 255.0,
                            240.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            227.0 / 255.0,
                            215.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            92.0 / 255.0,
                            180.0 / 255.0,
                            95.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            160.0 / 255.0,
                            123.0 / 255.0,
                            50.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            21.0 / 255.0,
                            20.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            244.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            220.0 / 255.0,
                            174.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            227.0 / 255.0,
                            180.0 / 255.0,
                            67.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            210.0 / 255.0,
                            208.0 / 255.0,
                            199.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            79.0 / 255.0,
                            77.0 / 255.0,
                            71.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            243.0 / 255.0,
                            241.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            231.0 / 255.0,
                            230.0 / 255.0,
                            226.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            224.0 / 255.0,
                            223.0 / 255.0,
                            215.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            138.0 / 255.0,
                            137.0 / 255.0,
                            131.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            252.0 / 255.0,
                            250.0 / 255.0,
                            0.501961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            252.0 / 255.0,
                            250.0 / 255.0,
                            0.619608,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            252.0 / 255.0,
                            250.0 / 255.0,
                            0.858824,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.129412,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.219608,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.301961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.219608,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.101961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            227.0 / 255.0,
                            225.0 / 255.0,
                            217.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            234.0 / 255.0,
                            227.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            250.0 / 255.0,
                            250.0 / 255.0,
                            248.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            251.0 / 255.0,
                            249.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            243.0 / 255.0,
                            242.0 / 255.0,
                            239.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            235.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            248.0 / 255.0,
                            247.0 / 255.0,
                            243.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            240.0 / 255.0,
                            239.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            214.0 / 255.0,
                            212.0 / 255.0,
                            204.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            239.0 / 255.0,
                            238.0 / 255.0,
                            233.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            9.0 / 255.0,
                            11.0 / 255.0,
                            14.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            36.0 / 255.0,
                            42.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            233.0 / 255.0,
                            225.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            214.0 / 255.0,
                            208.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            246.0 / 255.0,
                            244.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            163.0 / 255.0,
                            82.0 / 255.0,
                            72.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            143.0 / 255.0,
                            70.0 / 255.0,
                            61.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            50.0 / 255.0,
                            47.0 / 255.0,
                            40.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            58.0 / 255.0,
                            57.0 / 255.0,
                            52.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            154.0 / 255.0,
                            152.0 / 255.0,
                            143.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            167.0 / 255.0,
                            165.0 / 255.0,
                            155.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            179.0 / 255.0,
                            177.0 / 255.0,
                            168.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            189.0 / 255.0,
                            187.0 / 255.0,
                            177.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            203.0 / 255.0,
                            201.0 / 255.0,
                            191.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            123.0 / 255.0,
                            167.0 / 255.0,
                            140.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            95.0 / 255.0,
                            122.0 / 255.0,
                            158.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            238.0 / 255.0,
                            242.0 / 255.0,
                            247.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            218.0 / 255.0,
                            226.0 / 255.0,
                            236.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            154.0 / 255.0,
                            184.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            163.0 / 255.0,
                            82.0 / 255.0,
                            72.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            236.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            207.0 / 255.0,
                            201.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            106.0 / 255.0,
                            94.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            248.0 / 255.0,
                            240.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.341176,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            247.0 / 255.0,
                            246.0 / 255.0,
                            242.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            250.0 / 255.0,
                            249.0 / 255.0,
                            246.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            252.0 / 255.0,
                            251.0 / 255.0,
                            249.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            250.0 / 255.0,
                            247.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            248.0 / 255.0,
                            243.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            240.0 / 255.0,
                            236.0 / 255.0,
                            225.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            244.0 / 255.0,
                            231.0 / 255.0,
                            200.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            217.0 / 255.0,
                            216.0 / 255.0,
                            208.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            213.0 / 255.0,
                            211.0 / 255.0,
                            202.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            182.0 / 255.0,
                            180.0 / 255.0,
                            168.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            200.0 / 255.0,
                            198.0 / 255.0,
                            188.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            194.0 / 255.0,
                            192.0 / 255.0,
                            182.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            208.0 / 255.0,
                            206.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            220.0 / 255.0,
                            219.0 / 255.0,
                            212.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            122.0 / 255.0,
                            120.0 / 255.0,
                            114.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            126.0 / 255.0,
                            158.0 / 255.0,
                            136.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            102.0 / 255.0,
                            100.0 / 255.0,
                            94.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            122.0 / 255.0,
                            111.0 / 255.0,
                            158.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            241.0 / 255.0,
                            237.0 / 255.0,
                            245.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            221.0 / 255.0,
                            210.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            240.0 / 255.0,
                            245.0 / 255.0,
                            241.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            220.0 / 255.0,
                            235.0 / 255.0,
                            224.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            238.0 / 255.0,
                            246.0 / 255.0,
                            239.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            225.0 / 255.0,
                            239.0 / 255.0,
                            227.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            47.0 / 255.0,
                            107.0 / 255.0,
                            65.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            238.0 / 255.0,
                            236.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            244.0 / 255.0,
                            221.0 / 255.0,
                            216.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            161.0 / 255.0,
                            67.0 / 255.0,
                            56.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            246.0 / 255.0,
                            243.0 / 255.0,
                            249.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            74.0 / 255.0,
                            72.0 / 255.0,
                            67.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            224.0 / 255.0,
                            145.0 / 255.0,
                            138.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            160.0 / 255.0,
                            138.0 / 255.0,
                            90.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            95.0 / 255.0,
                            138.0 / 255.0,
                            114.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            237.0 / 255.0,
                            244.0 / 255.0,
                            239.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            122.0 / 255.0,
                            111.0 / 255.0,
                            158.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            241.0 / 255.0,
                            239.0 / 255.0,
                            247.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            74.0 / 255.0,
                            72.0 / 255.0,
                            67.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            242.0 / 255.0,
                            241.0 / 255.0,
                            237.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            185.0 / 255.0,
                            113.0 / 255.0,
                            78.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            250.0 / 255.0,
                            240.0 / 255.0,
                            233.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            192.0 / 255.0,
                            138.0 / 255.0,
                            62.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            250.0 / 255.0,
                            243.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                    ],
                }
            }
            AppTheme::AppDark => {
                Palette {
                    colors: [
                        ::ducktape_view_guest::wire::Rgba([
                            212.0 / 255.0,
                            210.0 / 255.0,
                            202.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            69.0 / 255.0,
                            68.0 / 255.0,
                            60.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            34.0 / 255.0,
                            33.0 / 255.0,
                            29.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            232.0 / 255.0,
                            230.0 / 255.0,
                            223.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            168.0 / 255.0,
                            166.0 / 255.0,
                            156.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            232.0 / 255.0,
                            230.0 / 255.0,
                            223.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            244.0 / 255.0,
                            242.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            50.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            107.0 / 255.0,
                            106.0 / 255.0,
                            97.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            41.0 / 255.0,
                            37.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            181.0 / 255.0,
                            179.0 / 255.0,
                            169.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            45.0 / 255.0,
                            39.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            205.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            201.0 / 255.0,
                            138.0 / 255.0,
                            99.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            38.0 / 255.0,
                            29.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            74.0 / 255.0,
                            56.0 / 255.0,
                            43.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            217.0 / 255.0,
                            123.0 / 255.0,
                            114.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            33.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            77.0 / 255.0,
                            47.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            224.0 / 255.0,
                            101.0 / 255.0,
                            92.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            184.0 / 255.0,
                            148.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            21.0 / 255.0,
                            20.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            42.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            50.0 / 255.0,
                            71.0 / 255.0,
                            58.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            92.0 / 255.0,
                            180.0 / 255.0,
                            95.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            212.0 / 255.0,
                            169.0 / 255.0,
                            78.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            21.0 / 255.0,
                            20.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            39.0 / 255.0,
                            23.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            77.0 / 255.0,
                            63.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            227.0 / 255.0,
                            180.0 / 255.0,
                            67.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            58.0 / 255.0,
                            57.0 / 255.0,
                            49.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            205.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            243.0 / 255.0,
                            241.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            53.0 / 255.0,
                            52.0 / 255.0,
                            46.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            59.0 / 255.0,
                            58.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            133.0 / 255.0,
                            131.0 / 255.0,
                            123.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            232.0 / 255.0,
                            230.0 / 255.0,
                            223.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            0.501961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            0.619608,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            0.858824,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.250980,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.349020,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.450980,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.349020,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.149020,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            18.0 / 255.0,
                            17.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            25.0 / 255.0,
                            24.0 / 255.0,
                            21.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            32.0 / 255.0,
                            31.0 / 255.0,
                            27.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            29.0 / 255.0,
                            25.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            41.0 / 255.0,
                            37.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            49.0 / 255.0,
                            48.0 / 255.0,
                            43.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            36.0 / 255.0,
                            35.0 / 255.0,
                            30.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            39.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            14.0 / 255.0,
                            13.0 / 255.0,
                            11.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            44.0 / 255.0,
                            43.0 / 255.0,
                            38.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            9.0 / 255.0,
                            11.0 / 255.0,
                            14.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            36.0 / 255.0,
                            42.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            48.0 / 255.0,
                            47.0 / 255.0,
                            41.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            77.0 / 255.0,
                            47.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            29.0 / 255.0,
                            27.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            194.0 / 255.0,
                            90.0 / 255.0,
                            79.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            211.0 / 255.0,
                            104.0 / 255.0,
                            92.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            244.0 / 255.0,
                            242.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            220.0 / 255.0,
                            218.0 / 255.0,
                            210.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            143.0 / 255.0,
                            141.0 / 255.0,
                            132.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            124.0 / 255.0,
                            122.0 / 255.0,
                            113.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            107.0 / 255.0,
                            106.0 / 255.0,
                            97.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            96.0 / 255.0,
                            95.0 / 255.0,
                            86.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            85.0 / 255.0,
                            84.0 / 255.0,
                            76.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            123.0 / 255.0,
                            167.0 / 255.0,
                            140.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            154.0 / 255.0,
                            184.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            37.0 / 255.0,
                            48.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            48.0 / 255.0,
                            62.0 / 255.0,
                            82.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            154.0 / 255.0,
                            184.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            211.0 / 255.0,
                            104.0 / 255.0,
                            92.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            48.0 / 255.0,
                            31.0 / 255.0,
                            28.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            77.0 / 255.0,
                            47.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            106.0 / 255.0,
                            94.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            37.0 / 255.0,
                            23.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.501961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            32.0 / 255.0,
                            31.0 / 255.0,
                            26.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            35.0 / 255.0,
                            34.0 / 255.0,
                            29.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            32.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            36.0 / 255.0,
                            24.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            34.0 / 255.0,
                            27.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            53.0 / 255.0,
                            50.0 / 255.0,
                            42.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            69.0 / 255.0,
                            58.0 / 255.0,
                            30.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            63.0 / 255.0,
                            62.0 / 255.0,
                            54.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            69.0 / 255.0,
                            68.0 / 255.0,
                            60.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            110.0 / 255.0,
                            109.0 / 255.0,
                            99.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            91.0 / 255.0,
                            90.0 / 255.0,
                            82.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            98.0 / 255.0,
                            97.0 / 255.0,
                            90.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            74.0 / 255.0,
                            73.0 / 255.0,
                            65.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            50.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            163.0 / 255.0,
                            161.0 / 255.0,
                            152.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            126.0 / 255.0,
                            158.0 / 255.0,
                            136.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            157.0 / 255.0,
                            155.0 / 255.0,
                            146.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            168.0 / 255.0,
                            154.0 / 255.0,
                            201.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            38.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            68.0 / 255.0,
                            60.0 / 255.0,
                            87.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            42.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            50.0 / 255.0,
                            71.0 / 255.0,
                            58.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            29.0 / 255.0,
                            42.0 / 255.0,
                            32.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            36.0 / 255.0,
                            53.0 / 255.0,
                            42.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            143.0 / 255.0,
                            201.0 / 255.0,
                            162.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            47.0 / 255.0,
                            31.0 / 255.0,
                            28.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            61.0 / 255.0,
                            39.0 / 255.0,
                            35.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            222.0 / 255.0,
                            139.0 / 255.0,
                            127.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            35.0 / 255.0,
                            48.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            45.0 / 255.0,
                            40.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            160.0 / 255.0,
                            92.0 / 255.0,
                            85.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            192.0 / 255.0,
                            168.0 / 255.0,
                            110.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            184.0 / 255.0,
                            148.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            42.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            168.0 / 255.0,
                            154.0 / 255.0,
                            201.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            38.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            205.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            45.0 / 255.0,
                            40.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            208.0 / 255.0,
                            144.0 / 255.0,
                            104.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            38.0 / 255.0,
                            29.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            212.0 / 255.0,
                            169.0 / 255.0,
                            78.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            39.0 / 255.0,
                            23.0 / 255.0,
                            1.000000,
                        ]),
                    ],
                }
            }
        }
    }
}
#[allow(unused_parens)]
impl NodeView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            node_data_dir: "".to_owned(),
            tier: "".to_owned(),
            admin: false,
            status: "".to_owned(),
            wall_now: 0,
            connected: false,
            connection_serial: 0,
            node_tab: NodeTab::Overview,
            facts: crate::host::empty_facts(),
            loading: true,
            module_rows: Vec::new(),
            node_peers: Vec::new(),
            log_lines: Vec::new(),
            node_log_filter: "".to_owned(),
            live_log_filter: "".to_owned(),
            live_filter_note: "".to_owned(),
            host_error: "".to_owned(),
            sent: false,
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    #[allow(clippy::too_many_arguments)]
    fn restore_state(
        active_palette: AppTheme,
        node_data_dir: String,
        tier: String,
        admin: bool,
        status: String,
        wall_now: i64,
        connected: bool,
        connection_serial: i64,
        node_tab: NodeTab,
        facts: crate::host::NodeFacts,
        loading: bool,
        module_rows: Vec<crate::host::ModuleRow>,
        node_peers: Vec<crate::host::PeerRow>,
        log_lines: Vec<crate::host::LogRow>,
        node_log_filter: String,
        live_log_filter: String,
        live_filter_note: String,
        host_error: String,
        sent: bool,
    ) -> Self {
        Self {
            active_palette: active_palette,
            node_data_dir: node_data_dir,
            tier: tier,
            admin: admin,
            status: status,
            wall_now: wall_now,
            connected: connected,
            connection_serial: connection_serial,
            node_tab: node_tab,
            facts: facts,
            loading: loading,
            module_rows: module_rows,
            node_peers: node_peers,
            log_lines: log_lines,
            node_log_filter: node_log_filter,
            live_log_filter: live_log_filter,
            live_filter_note: live_filter_note,
            host_error: host_error,
            sent: sent,
        }
    }
    pub(crate) const SNAPSHOT_SCHEMA: &'static str = "2aca5358ae16aabea0acc404d80c7acb4614980b4df1b00c1feb863a13525343";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("NodeView"),
                fields: vec![
                    (String::from("active_palette"), match & self.active_palette {
                    AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("AppTheme"), fields : vec![(String::from("app"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    AppTheme::AppDark =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AppTheme"), fields : vec![(String::from("app_dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("node_data_dir"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.node_data_dir))), (String::from("tier"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.tier))), (String::from("admin"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.admin))),
                    (String::from("status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.status))), (String::from("wall_now"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .wall_now))), (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected))), (String::from("connection_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .connection_serial))), (String::from("node_tab"), match & self
                    .node_tab { NodeTab::Overview =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("NodeTab"), fields : vec![(String::from("overview"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    NodeTab::Permissions =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("NodeTab"), fields : vec![(String::from("permissions"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    NodeTab::Activity =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("NodeTab"), fields : vec![(String::from("activity"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    NodeTab::Modules =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("NodeTab"), fields : vec![(String::from("modules"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("facts"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("NodeFacts"), fields :
                    ::std::vec![(String::from("node_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.facts).node_key))), (String::from("node_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self.facts)
                    .node_height))), (String::from("node_checkpoint"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self.facts)
                    .node_checkpoint))), (String::from("node_last_finalized"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self.facts)
                    .node_last_finalized))), (String::from("node_reachable_label"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.facts).node_reachable_label))),
                    (String::from("node_quorum_label"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.facts).node_quorum_label))), (String::from("node_version"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.facts).node_version))), (String::from("node_root_hash"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.facts).node_root_hash))), (String::from("sync_line"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.facts).sync_line))), (String::from("node_phase_since"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self.facts)
                    .node_phase_since))), (String::from("node_sync_retries"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self.facts)
                    .node_sync_retries))), (String::from("node_sync_failures"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self.facts)
                    .node_sync_failures))), (String::from("node_sync_last_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.facts).node_sync_last_error)))] }), (String::from("loading"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .loading))), (String::from("module_rows"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.module_rows)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ModuleRow"), fields : ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("category"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).category))), (String::from("root"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).root))), (String::from("code_hash"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).code_hash))), (String::from("pending_hash"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).pending_hash))), (String::from("activation_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .activation_height))), (String::from("readiness"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .readiness))), (String::from("ready"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .ready)))] }).collect())), (String::from("node_peers"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.node_peers)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("PeerRow"), fields : ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).key))), (String::from("role"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).role))), (String::from("live"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).live)))]
                    }).collect())), (String::from("log_lines"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.log_lines)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("LogRow"), fields : ::std::vec![(String::from("cursor"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).cursor))), (String::from("time"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).time))), (String::from("level"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).level))), (String::from("message"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).message)))] }).collect())), (String::from("node_log_filter"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.node_log_filter))), (String::from("live_log_filter"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.live_log_filter))), (String::from("live_filter_note"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.live_filter_note))), (String::from("host_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.host_error))), (String::from("sent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.sent)))
                ],
            },
        }
            .encode()
    }
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, String> {
        let snapshot = ::ducktape_view_guest::wire::Snapshot::decode(bytes)?;
        if snapshot.schema != Self::SNAPSHOT_SCHEMA {
            return Err(String::from("snapshot schema mismatch"));
        }
        let value = snapshot.state;
        ((|| {
            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: name,
                fields: fields,
            } = value else {
                return None;
            };
            if name != "NodeView" || fields.len() != 19 {
                return None;
            }
            let mut fields = fields.into_iter();
            let (name, value) = fields.next()?;
            if name != "active_palette" {
                return None;
            }
            let active_palette: AppTheme = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "AppTheme" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "app" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(AppTheme::App)
                    }
                    "app_dark" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(AppTheme::AppDark)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "node_data_dir" {
                return None;
            }
            let node_data_dir: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tier" {
                return None;
            }
            let tier: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "admin" {
                return None;
            }
            let admin: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "status" {
                return None;
            }
            let status: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "wall_now" {
                return None;
            }
            let wall_now: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "connected" {
                return None;
            }
            let connected: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "connection_serial" {
                return None;
            }
            let connection_serial: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "node_tab" {
                return None;
            }
            let node_tab: NodeTab = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "NodeTab" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "overview" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(NodeTab::Overview)
                    }
                    "permissions" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(NodeTab::Permissions)
                    }
                    "activity" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(NodeTab::Activity)
                    }
                    "modules" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(NodeTab::Modules)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "facts" {
                return None;
            }
            let facts: crate::host::NodeFacts = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "NodeFacts" || fields.len() != 13 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "node_key" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "node_height" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "node_checkpoint" {
                    return None;
                }
                let (name, field_3) = fields.next()?;
                if name != "node_last_finalized" {
                    return None;
                }
                let (name, field_4) = fields.next()?;
                if name != "node_reachable_label" {
                    return None;
                }
                let (name, field_5) = fields.next()?;
                if name != "node_quorum_label" {
                    return None;
                }
                let (name, field_6) = fields.next()?;
                if name != "node_version" {
                    return None;
                }
                let (name, field_7) = fields.next()?;
                if name != "node_root_hash" {
                    return None;
                }
                let (name, field_8) = fields.next()?;
                if name != "sync_line" {
                    return None;
                }
                let (name, field_9) = fields.next()?;
                if name != "node_phase_since" {
                    return None;
                }
                let (name, field_10) = fields.next()?;
                if name != "node_sync_retries" {
                    return None;
                }
                let (name, field_11) = fields.next()?;
                if name != "node_sync_failures" {
                    return None;
                }
                let (name, field_12) = fields.next()?;
                if name != "node_sync_last_error" {
                    return None;
                }
                Some(crate::host::NodeFacts {
                    node_key: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_height: (match field_1 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_checkpoint: (match field_2 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_last_finalized: (match field_3 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_reachable_label: (match field_4 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_quorum_label: (match field_5 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_version: (match field_6 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_root_hash: (match field_7 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    sync_line: (match field_8 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_phase_since: (match field_9 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_sync_retries: (match field_10 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_sync_failures: (match field_11 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    node_sync_last_error: (match field_12 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "loading" {
                return None;
            }
            let loading: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "module_rows" {
                return None;
            }
            let module_rows: Vec<crate::host::ModuleRow> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
                                return None;
                            };
                            if name != "ModuleRow" || fields.len() != 8 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "category" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "root" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "code_hash" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "pending_hash" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "activation_height" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "readiness" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "ready" {
                                return None;
                            }
                            Some(crate::host::ModuleRow {
                                id: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                category: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                root: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                code_hash: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                pending_hash: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                activation_height: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                readiness: (match field_6 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                ready: (match field_7 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "node_peers" {
                return None;
            }
            let node_peers: Vec<crate::host::PeerRow> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
                                return None;
                            };
                            if name != "PeerRow" || fields.len() != 3 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "key" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "role" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "live" {
                                return None;
                            }
                            Some(crate::host::PeerRow {
                                key: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                role: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                live: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "log_lines" {
                return None;
            }
            let log_lines: Vec<crate::host::LogRow> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
                                return None;
                            };
                            if name != "LogRow" || fields.len() != 4 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "cursor" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "time" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "level" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "message" {
                                return None;
                            }
                            Some(crate::host::LogRow {
                                cursor: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                time: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                level: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                message: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "node_log_filter" {
                return None;
            }
            let node_log_filter: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "live_log_filter" {
                return None;
            }
            let live_log_filter: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "live_filter_note" {
                return None;
            }
            let live_filter_note: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "host_error" {
                return None;
            }
            let host_error: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sent" {
                return None;
            }
            let sent: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            Some(
                Self::restore_state(
                    active_palette,
                    node_data_dir,
                    tier,
                    admin,
                    status,
                    wall_now,
                    connected,
                    connection_serial,
                    node_tab,
                    facts,
                    loading,
                    module_rows,
                    node_peers,
                    log_lines,
                    node_log_filter,
                    live_log_filter,
                    live_filter_note,
                    host_error,
                    sent,
                ),
            )
        })())
            .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl NodeView {
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::facts(self.connection_serial)
                        .map(move |value| Message::FactsArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (self.node_tab == NodeTab::Overview)) {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::peers(self.connection_serial)
                        .map(move |value| Message::PeersArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (self.node_tab == NodeTab::Modules)) {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::modules(self.connection_serial)
                        .map(move |value| Message::ModulesArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (self.node_tab == NodeTab::Activity)) {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::logs(self.connection_serial)
                        .map(move |value| Message::LogsArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            crate::host::acts().map(move |value| Message::ActDone(value)),
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn view_fits_default_stack() {
        ::std::thread::Builder::new()
            .stack_size(4 * 1024 * 1024)
            .spawn(|| {
                let (app, _) = NodeView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
impl NodeView {
    pub(crate) fn render_node_body(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/node-body", use_scope);
            ::ducktape_view_guest::wire::Node::Scroll {
                on_scroll: None,
                virtual_rows: false,
                key: node_scope.clone(),
                direction: ::ducktape_view_guest::wire::ScrollDirection::Vertical,
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                bar_hidden: false,
                bar_width: None,
                bar_margin: None,
                scroller_width: None,
                bar_spacing: None,
                anchor_x: ::ducktape_view_guest::wire::ScrollAnchor::Start,
                anchor_y: ::ducktape_view_guest::wire::ScrollAnchor::Start,
                auto_scroll: (false),
                background: None,
                border: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Text {
                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                height: None,
                                                align_y: None,
                                                line_height: None,
                                                shaping: None,
                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                tracking: 0.0f32,
                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                        "Geist".into(),
                                                    ),
                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:248", use_scope),
                                            size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[7]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: ("This node".to_owned()).to_string(),
                                        });
                                    children
                                        .push(
                                            self
                                                .render_status_pill_1(
                                                    palette,
                                                    format!("{}/StatusPill@2118", use_scope),
                                                ),
                                        );
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Space {
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:243", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                        spacing: Some((10.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push({
                                            let node_scope = format!(
                                                "{}/node-overview-tab", node_scope
                                            );
                                            ::ducktape_view_guest::wire::Node::Button {
                                                checked: Some((self.node_tab == NodeTab::Overview)),
                                                expanded: None,
                                                description: None,
                                                key: node_scope.clone(),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                    Box::new(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:263", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (0.0) as f32,
                                                            right: (15.0) as f32,
                                                            bottom: (0.0) as f32,
                                                            left: (15.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (None)
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            self
                                                                .render_tab_label_2(
                                                                    palette,
                                                                    format!("{}/TabLabel@2128", use_scope),
                                                                ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(String::from("Node overview".to_owned())),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        (move |event_0| Message::SelectNodeTab(
                                                            event_0,
                                                        ))(NodeTab::Overview),
                                                    ),
                                                ),
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[5]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[57]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            }
                                        });
                                    children
                                        .push({
                                            let node_scope = format!(
                                                "{}/node-permissions-tab", node_scope
                                            );
                                            ::ducktape_view_guest::wire::Node::Button {
                                                checked: Some((self.node_tab == NodeTab::Permissions)),
                                                expanded: None,
                                                description: None,
                                                key: node_scope.clone(),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                    Box::new(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:278", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (0.0) as f32,
                                                            right: (15.0) as f32,
                                                            bottom: (0.0) as f32,
                                                            left: (15.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (None)
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            self
                                                                .render_tab_label_3(
                                                                    palette,
                                                                    format!("{}/TabLabel@2143", use_scope),
                                                                ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(String::from("Node permissions".to_owned())),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        (move |event_0| Message::SelectNodeTab(
                                                            event_0,
                                                        ))(NodeTab::Permissions),
                                                    ),
                                                ),
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[5]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[57]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            }
                                        });
                                    children
                                        .push({
                                            let node_scope = format!(
                                                "{}/node-activity-tab", node_scope
                                            );
                                            ::ducktape_view_guest::wire::Node::Button {
                                                checked: Some((self.node_tab == NodeTab::Activity)),
                                                expanded: None,
                                                description: None,
                                                key: node_scope.clone(),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                    Box::new(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:293", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (0.0) as f32,
                                                            right: (15.0) as f32,
                                                            bottom: (0.0) as f32,
                                                            left: (15.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (None)
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            self
                                                                .render_tab_label_4(
                                                                    palette,
                                                                    format!("{}/TabLabel@2158", use_scope),
                                                                ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(String::from("Node activity".to_owned())),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        (move |event_0| Message::SelectNodeTab(
                                                            event_0,
                                                        ))(NodeTab::Activity),
                                                    ),
                                                ),
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[5]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[57]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            }
                                        });
                                    children
                                        .push({
                                            let node_scope = format!("{}/node-modules-tab", node_scope);
                                            ::ducktape_view_guest::wire::Node::Button {
                                                checked: Some((self.node_tab == NodeTab::Modules)),
                                                expanded: None,
                                                description: None,
                                                key: node_scope.clone(),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                    Box::new(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:308", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (0.0) as f32,
                                                            right: (15.0) as f32,
                                                            bottom: (0.0) as f32,
                                                            left: (15.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (None)
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            self
                                                                .render_tab_label_5(
                                                                    palette,
                                                                    format!("{}/TabLabel@2173", use_scope),
                                                                ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(String::from("Node modules".to_owned())),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::OpenNodeModules,
                                                    ),
                                                ),
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[5]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[57]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            }
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:256", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                        spacing: Some((3.0) as f32),
                                        padding: None,
                                        width: None,
                                        height: None,
                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                            match &(self.node_tab) {
                                NodeTab::Modules => {
                                    children
                                        .push(
                                            self
                                                .render_modules_panel_17(
                                                    palette,
                                                    format!("{}/ModulesPanel@2183", use_scope),
                                                ),
                                        );
                                }
                                NodeTab::Permissions => {
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(
                                                    self
                                                        .render_node_access_card_40(
                                                            palette,
                                                            format!("{}/NodeAccessCard@2186", use_scope),
                                                        ),
                                                );
                                            children
                                                .push(
                                                    self
                                                        .render_permission_matrix_55(
                                                            palette,
                                                            format!("{}/PermissionMatrix@2187", use_scope),
                                                        ),
                                                );
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:321", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((18.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: None,
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        });
                                }
                                NodeTab::Activity => {
                                    children
                                        .push(
                                            self
                                                .render_log_timeline_frame_59(
                                                    palette,
                                                    format!("{}/LogTimeline.Frame@2189", use_scope),
                                                    (move || Message::ApplyLiveLogFilter).clone(),
                                                    (move |event_0, event_1| Message::CopyToClipboard(
                                                        event_0,
                                                        event_1,
                                                    ))
                                                        .clone(),
                                                    (move |event_0| Message::LiveLogFilterChanged(event_0))
                                                        .clone(),
                                                    (move |event_0| Message::NodeLogFilterChanged(event_0))
                                                        .clone(),
                                                    (move || Message::OpenNodeModules).clone(),
                                                    (move |event_0| Message::SelectNodeTab(event_0)).clone(),
                                                ),
                                        );
                                }
                                NodeTab::Overview => {
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(
                                                    self
                                                        .render_group_label_60(
                                                            palette,
                                                            format!("{}/GroupLabel@2240", use_scope),
                                                        ),
                                                );
                                            children
                                                .push(
                                                    self
                                                        .render_member_fact_row_61(
                                                            palette,
                                                            format!("{}/MemberFactRow@2241", use_scope),
                                                        ),
                                                );
                                            children
                                                .push(
                                                    self
                                                        .render_member_fact_row_62(
                                                            palette,
                                                            format!("{}/MemberFactRow@2242", use_scope),
                                                        ),
                                                );
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                    checked: None,
                                                    expanded: None,
                                                    description: None,
                                                    key: format!("{}/@button:382", use_scope),
                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                        String::from("Copy node key"),
                                                    ),
                                                    label: None,
                                                    on_press: if ((self.facts.node_key).is_empty()) {
                                                        None
                                                    } else {
                                                        Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    (move |event_0, event_1| Message::CopyToClipboard(
                                                                        event_0,
                                                                        event_1,
                                                                    ))(
                                                                        self.facts.node_key.to_owned(),
                                                                        "Node key copied".to_owned(),
                                                                    ),
                                                                ),
                                                            )
                                                    },
                                                    width: None,
                                                    height: None,
                                                    padding: Some(
                                                        ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
                                                    ),
                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                            base: ::ducktape_view_guest::wire::Face {
                                                                background: Some(palette.colors[12]),
                                                                text: Some(palette.colors[13]),
                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                    color: Some(palette.colors[40]),
                                                                    width: Some(1.0),
                                                                    radius: Some([9.0; 4]),
                                                                }),
                                                            },
                                                            hover_background: Some(palette.colors[14]),
                                                            pressed_background: Some(palette.colors[6]),
                                                            disabled_background: None,
                                                            disabled_text: None,
                                                            disabled_opacity: Some(0.5f32),
                                                            focus_ring: Some(palette.colors[42]),
                                                            text_size: Some(12.5f32),
                                                            line_height: None,
                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                    "Geist".into(),
                                                                ),
                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        }),
                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                        hovered: None,
                                                        pressed: None,
                                                        disabled: None,
                                                    },
                                                });
                                            children
                                                .push(
                                                    self
                                                        .render_group_label_63(
                                                            palette,
                                                            format!("{}/GroupLabel@2251", use_scope),
                                                        ),
                                                );
                                            children
                                                .push({
                                                    let mut items = Vec::new();
                                                    let min_cell = ((170.0) as f32)
                                                        .max(f32::EPSILON)
                                                        .min(f32::MAX);
                                                    let flex_child: ::ducktape_view_guest::wire::Node = self
                                                        .render_stat_card_64(
                                                            palette,
                                                            format!("{}/StatCard@2257", use_scope),
                                                        );
                                                    items
                                                        .push((
                                                            ::ducktape_view_guest::wire::FlexItem {
                                                                grow: Some(1.0),
                                                                shrink: 0.0,
                                                                basis: ::ducktape_view_guest::wire::FlexBasis::Fixed(
                                                                    min_cell,
                                                                ),
                                                                ..Default::default()
                                                            },
                                                            flex_child,
                                                        ));
                                                    let flex_child: ::ducktape_view_guest::wire::Node = self
                                                        .render_stat_card_65(
                                                            palette,
                                                            format!("{}/StatCard@2262", use_scope),
                                                        );
                                                    items
                                                        .push((
                                                            ::ducktape_view_guest::wire::FlexItem {
                                                                grow: Some(1.0),
                                                                shrink: 0.0,
                                                                basis: ::ducktape_view_guest::wire::FlexBasis::Fixed(
                                                                    min_cell,
                                                                ),
                                                                ..Default::default()
                                                            },
                                                            flex_child,
                                                        ));
                                                    let flex_child: ::ducktape_view_guest::wire::Node = self
                                                        .render_stat_card_66(
                                                            palette,
                                                            format!("{}/StatCard@2267", use_scope),
                                                        );
                                                    items
                                                        .push((
                                                            ::ducktape_view_guest::wire::FlexItem {
                                                                grow: Some(1.0),
                                                                shrink: 0.0,
                                                                basis: ::ducktape_view_guest::wire::FlexBasis::Fixed(
                                                                    min_cell,
                                                                ),
                                                                ..Default::default()
                                                            },
                                                            flex_child,
                                                        ));
                                                    let (items, children) = items.into_iter().unzip();
                                                    ::ducktape_view_guest::wire::Node::Flex {
                                                        key: format!("{}/@layout:392", use_scope),
                                                        items,
                                                        children,
                                                        background: None,
                                                        border: None,
                                                        layout: ::ducktape_view_guest::wire::FlexLayout {
                                                            direction: ::ducktape_view_guest::wire::FlexDirection::Row,
                                                            wrap: ::ducktape_view_guest::wire::FlexWrap::Wrap,
                                                            justify: None,
                                                            items: None,
                                                            content: None,
                                                            row_gap: Some((10.0) as f32),
                                                            column_gap: Some((10.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            max_width: None,
                                                            max_height: None,
                                                            clip: (false),
                                                            surface_width: None,
                                                            surface_height: None,
                                                            surface_max_width: None,
                                                        },
                                                    }
                                                });
                                            if self.admin {
                                                children
                                                    .push({
                                                        let mut items = Vec::new();
                                                        let min_cell = ((170.0) as f32)
                                                            .max(f32::EPSILON)
                                                            .min(f32::MAX);
                                                        let flex_child: ::ducktape_view_guest::wire::Node = self
                                                            .render_stat_card_67(
                                                                palette,
                                                                format!("{}/StatCard@2274", use_scope),
                                                            );
                                                        items
                                                            .push((
                                                                ::ducktape_view_guest::wire::FlexItem {
                                                                    grow: Some(1.0),
                                                                    shrink: 0.0,
                                                                    basis: ::ducktape_view_guest::wire::FlexBasis::Fixed(
                                                                        min_cell,
                                                                    ),
                                                                    ..Default::default()
                                                                },
                                                                flex_child,
                                                            ));
                                                        let (items, children) = items.into_iter().unzip();
                                                        ::ducktape_view_guest::wire::Node::Flex {
                                                            key: format!("{}/@layout:409", use_scope),
                                                            items,
                                                            children,
                                                            background: None,
                                                            border: None,
                                                            layout: ::ducktape_view_guest::wire::FlexLayout {
                                                                direction: ::ducktape_view_guest::wire::FlexDirection::Row,
                                                                wrap: ::ducktape_view_guest::wire::FlexWrap::Wrap,
                                                                justify: None,
                                                                items: None,
                                                                content: None,
                                                                row_gap: Some((10.0) as f32),
                                                                column_gap: Some((10.0) as f32),
                                                                padding: None,
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                height: None,
                                                                max_width: None,
                                                                max_height: None,
                                                                clip: (false),
                                                                surface_width: None,
                                                                surface_height: None,
                                                                surface_max_width: None,
                                                            },
                                                        }
                                                    });
                                            }
                                            children
                                                .push(
                                                    self
                                                        .render_group_card_75(
                                                            palette,
                                                            format!("{}/GroupCard@2279", use_scope),
                                                            (move || Message::ApplyLiveLogFilter).clone(),
                                                            (move |event_0, event_1| Message::CopyToClipboard(
                                                                event_0,
                                                                event_1,
                                                            ))
                                                                .clone(),
                                                            (move |event_0| Message::LiveLogFilterChanged(event_0))
                                                                .clone(),
                                                            (move |event_0| Message::NodeLogFilterChanged(event_0))
                                                                .clone(),
                                                            (move || Message::OpenNodeModules).clone(),
                                                            (move |event_0| Message::SelectNodeTab(event_0)).clone(),
                                                        ),
                                                );
                                            if (!(self.node_peers).is_empty()) {
                                                children
                                                    .push({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        children
                                                            .push(
                                                                self
                                                                    .render_group_label_76(
                                                                        palette,
                                                                        format!("{}/GroupLabel@2312", use_scope),
                                                                    ),
                                                            );
                                                        children
                                                            .push(
                                                                self
                                                                    .render_group_card_77(
                                                                        palette,
                                                                        format!("{}/GroupCard@2313", use_scope),
                                                                        (move || Message::ApplyLiveLogFilter).clone(),
                                                                        (move |event_0, event_1| Message::CopyToClipboard(
                                                                            event_0,
                                                                            event_1,
                                                                        ))
                                                                            .clone(),
                                                                        (move |event_0| Message::LiveLogFilterChanged(event_0))
                                                                            .clone(),
                                                                        (move |event_0| Message::NodeLogFilterChanged(event_0))
                                                                            .clone(),
                                                                        (move || Message::OpenNodeModules).clone(),
                                                                        (move |event_0| Message::SelectNodeTab(event_0)).clone(),
                                                                    ),
                                                            );
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:447", use_scope),
                                                            wrap: None,
                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                            spacing: Some((9.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            align: None,
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    });
                                            }
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:375", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((13.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: None,
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        });
                                }
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:242", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((13.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:237", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((18.0) as f32),
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (22.0) as f32,
                            right: (22.0) as f32,
                            bottom: (22.0) as f32,
                            left: (22.0) as f32,
                        }),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
}
impl NodeView {
    #[allow(clippy::assign_op_pattern)]
    pub(crate) fn update(
        &mut self,
        message: Message,
    ) -> ::ducktape_view_guest::Task<Message> {
        match message {
            Message::SessionArrived(item) => self.on_session_arrived(item),
            Message::FactsArrived(item) => self.on_facts_arrived(item),
            Message::PeersArrived(item) => self.on_peers_arrived(item),
            Message::ModulesArrived(item) => self.on_modules_arrived(item),
            Message::LogsArrived(item) => self.on_logs_arrived(item),
            Message::ActDone(item) => self.on_act_done(item),
            Message::SelectNodeTab(next) => self.on_select_node_tab(next),
            Message::OpenNodeModules => self.on_open_node_modules(),
            Message::NodeLogFilterChanged(next) => self.on_node_log_filter_changed(next),
            Message::LiveLogFilterChanged(next) => self.on_live_log_filter_changed(next),
            Message::ApplyLiveLogFilter => self.on_apply_live_log_filter(),
            Message::CopyToClipboard(text, label) => {
                self.on_copy_to_clipboard(text, label)
            }
            Message::BindNodeLogFilter(value) => self.on_bind_node_log_filter(value),
            Message::BindLiveLogFilter(value) => self.on_bind_live_log_filter(value),
        }
    }
    fn on_session_arrived(
        &mut self,
        item: crate::host::SessionItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = item.error.to_owned();
                self.host_error = next;
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            let next = item.next.clone();
            {
                let next = crate::host::connection_serial_after(
                    self.connected,
                    next.connected,
                    self.connection_serial,
                );
                self.connection_serial = next;
            }
            {
                let next = next.connected;
                self.connected = next;
            }
            {
                let next = next.admin;
                self.admin = next;
            }
            {
                let next = next.tier.to_owned();
                self.tier = next;
            }
            {
                let next = next.status.to_owned();
                self.status = next;
            }
            {
                let next = next.data_dir.to_owned();
                self.node_data_dir = next;
            }
            {
                let next = next.wall_now;
                self.wall_now = next;
            }
            {
                let next = AppTheme::App;
                self.active_palette = next;
            }
            if (!next.dark) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                let next = AppTheme::AppDark;
                self.active_palette = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_facts_arrived(
        &mut self,
        item: crate::host::FactsItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = item.error.to_owned();
                self.host_error = next;
            }
            {
                let next = false;
                self.loading = next;
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                let next = item.facts.clone();
                self.facts = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_peers_arrived(
        &mut self,
        item: crate::host::PeersItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = item.error.to_owned();
                self.host_error = next;
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                let next = item.rows.clone();
                self.node_peers = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_modules_arrived(
        &mut self,
        item: crate::host::ModulesItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = item.error.to_owned();
                self.host_error = next;
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                let next = item.rows.clone();
                self.module_rows = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_logs_arrived(
        &mut self,
        item: crate::host::LogItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = item.error.to_owned();
                self.host_error = next;
            }
            {
                let next = crate::host::push_logs(
                    ::std::convert::AsRef::as_ref(&(self.log_lines)),
                    ::std::convert::AsRef::as_ref(&(item.lines)),
                );
                self.log_lines = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_act_done(
        &mut self,
        item: crate::host::ActItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = item.error.to_owned();
                self.host_error = next;
            }
            {
                let next = crate::host::keep_str(
                    (item.error).is_empty(),
                    ::std::convert::AsRef::as_ref(&(item.reply)),
                    ::std::convert::AsRef::as_ref(&(item.error)),
                );
                self.live_filter_note = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_select_node_tab(
        &mut self,
        next: NodeTab,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = next.clone();
                self.node_tab = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_node_modules(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = NodeTab::Modules;
                self.node_tab = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_node_log_filter_changed(
        &mut self,
        next: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = next.to_owned();
                self.node_log_filter = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_live_log_filter_changed(
        &mut self,
        next: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = next.to_owned();
                self.live_log_filter = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_apply_live_log_filter(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((!self.admin) || (self.live_log_filter).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                let next = "".to_owned();
                self.live_filter_note = next;
            }
            {
                let next = (crate::host::set_log_filter(
                    ::std::convert::AsRef::as_ref(&(self.live_log_filter)),
                ));
                self.sent = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_copy_to_clipboard(
        &mut self,
        text: String,
        label: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = crate::host::copy(
                    ::std::convert::AsRef::as_ref(&(text)),
                    ::std::convert::AsRef::as_ref(&(label)),
                );
                self.sent = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_node_log_filter(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = value;
                self.node_log_filter = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_live_log_filter(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = value;
                self.live_log_filter = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
}
impl NodeView {
    pub(crate) fn view(&self) -> ::ducktape_view_guest::wire::Node {
        let palette = self.palette();
        {
            let node_scope = format!("{}/root", "NodeView");
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[2]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    if (!(self.host_error).is_empty()) {
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:191", node_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (13.0) as f32,
                                    right: (22.0) as f32,
                                    bottom: (0.0) as f32,
                                    left: (22.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new({
                                    let node_scope = format!("{}/host-error", node_scope);
                                    ::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: None,
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: node_scope.clone(),
                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[20]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (self.host_error.to_owned()).to_string(),
                                    }
                                }),
                            });
                    }
                    if (!self.connected) {
                        children
                            .push({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                children
                                    .push(::ducktape_view_guest::wire::Node::Space {
                                        width: None,
                                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                                    });
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: None,
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:200", node_scope),
                                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[5]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("Not connected".to_owned()).to_string(),
                                    });
                                children
                                    .push(::ducktape_view_guest::wire::Node::Space {
                                        width: None,
                                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:194", node_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                    spacing: None,
                                    padding: None,
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            });
                    }
                    if self.connected {
                        children
                            .push({
                                let node_scope = format!("{}/node", node_scope);
                                self.render_node_body(palette, node_scope.clone())
                            });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:187", node_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: None,
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
}
impl NodeView {
    pub(crate) fn render_icon_27(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                ();
                ();
                ();
                ();
                ();
                {
                    children
                        .push({
                            let (hash, bytes) = ::ducktape_view_guest::slots::picture(
                                crate::host::icon(::std::convert::AsRef::as_ref(&("lock"))),
                            );
                            ::ducktape_view_guest::wire::Node::Svg {
                                inherit_button_ink: false,
                                key: format!("{}/@media:46", use_scope),
                                hash: hash,
                                bytes: bytes,
                                label: None,
                                color: Some(palette.colors[74]),
                                hover: None,
                                fit: None,
                                rotation: None,
                                opacity: None,
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((11.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((11.0) as f32),
                                ),
                            }
                        });
                }
                ();
                ();
                ();
                ();
                ();
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
}
impl NodeView {
    pub(crate) fn render_status_dot_0(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if crate::host::connection_degraded(
                    ::std::convert::AsRef::as_ref(&(self.status)),
                ) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:271", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[83]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if ((!crate::host::connection_degraded(
                    ::std::convert::AsRef::as_ref(&(self.status)),
                )) && self.loading)
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:279", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[34]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if ((!crate::host::connection_degraded(
                    ::std::convert::AsRef::as_ref(&(self.status)),
                )) && (!self.loading))
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:287", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[29]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                    (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_status_pill_1(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (3.0) as f32,
                    right: (8.0) as f32,
                    bottom: (3.0) as f32,
                    left: (8.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[39]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(
                            self
                                .render_status_dot_0(
                                    palette,
                                    format!("{}/StatusDot@1718", use_scope),
                                ),
                        );
                    if crate::host::connection_degraded(
                        ::std::convert::AsRef::as_ref(&(self.status)),
                    ) {
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist Mono".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:131", use_scope),
                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[41]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Stopped".to_owned()).to_string(),
                            });
                    }
                    if ((!crate::host::connection_degraded(
                        ::std::convert::AsRef::as_ref(&(self.status)),
                    )) && self.loading)
                    {
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist Mono".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:138", use_scope),
                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[41]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Syncing…".to_owned()).to_string(),
                            });
                    }
                    if ((!crate::host::connection_degraded(
                        ::std::convert::AsRef::as_ref(&(self.status)),
                    )) && (!self.loading))
                    {
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist Mono".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:145", use_scope),
                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[41]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Synced".to_owned()).to_string(),
                            });
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:124", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((5.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_tab_label_2(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if (self.node_tab == NodeTab::Overview) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:161", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Overview".to_owned()).to_string(),
                                });
                        }
                        if (!(self.node_tab == NodeTab::Overview)) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:168", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Overview".to_owned()).to_string(),
                                });
                        }
                        if (0 > 0) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:175", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (1.0) as f32,
                                        right: (7.0) as f32,
                                        bottom: (1.0) as f32,
                                        left: (7.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:181", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (0).to_string(),
                                    }),
                                });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:154", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((7.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                if (self.node_tab == NodeTab::Overview) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:188", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if (!(self.node_tab == NodeTab::Overview)) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:195", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_tab_label_3(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if (self.node_tab == NodeTab::Permissions) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:161", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Permissions".to_owned()).to_string(),
                                });
                        }
                        if (!(self.node_tab == NodeTab::Permissions)) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:168", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Permissions".to_owned()).to_string(),
                                });
                        }
                        if (0 > 0) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:175", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (1.0) as f32,
                                        right: (7.0) as f32,
                                        bottom: (1.0) as f32,
                                        left: (7.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:181", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (0).to_string(),
                                    }),
                                });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:154", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((7.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                if (self.node_tab == NodeTab::Permissions) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:188", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if (!(self.node_tab == NodeTab::Permissions)) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:195", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_tab_label_4(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if (self.node_tab == NodeTab::Activity) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:161", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Activity".to_owned()).to_string(),
                                });
                        }
                        if (!(self.node_tab == NodeTab::Activity)) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:168", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Activity".to_owned()).to_string(),
                                });
                        }
                        if (0 > 0) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:175", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (1.0) as f32,
                                        right: (7.0) as f32,
                                        bottom: (1.0) as f32,
                                        left: (7.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:181", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (0).to_string(),
                                    }),
                                });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:154", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((7.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                if (self.node_tab == NodeTab::Activity) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:188", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if (!(self.node_tab == NodeTab::Activity)) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:195", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_tab_label_5(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if (self.node_tab == NodeTab::Modules) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:161", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Modules".to_owned()).to_string(),
                                });
                        }
                        if (!(self.node_tab == NodeTab::Modules)) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:168", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Modules".to_owned()).to_string(),
                                });
                        }
                        if (((self.module_rows).len() as i64) > 0) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:175", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (1.0) as f32,
                                        right: (7.0) as f32,
                                        bottom: (1.0) as f32,
                                        left: (7.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:181", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (((self.module_rows).len() as i64)).to_string(),
                                    }),
                                });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:154", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((7.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                if (self.node_tab == NodeTab::Modules) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:188", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if (!(self.node_tab == NodeTab::Modules)) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:195", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_group_label_6(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("REGISTERED".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_empty_plate_7(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (30.0) as f32,
                    right: (30.0) as f32,
                    bottom: (30.0) as f32,
                    left: (30.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: None,
                background: (Some(
                    ::ducktape_view_guest::wire::Rgba([
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.000000,
                    ]),
                ))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[39]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((12.0) as f32).max(0.0).min(f32::MAX),
                        ((12.0) as f32).max(0.0).min(f32::MAX),
                        ((12.0) as f32).max(0.0).min(f32::MAX),
                        ((12.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: None,
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                "Geist".into(),
                            ),
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:75", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("The node has not answered with its module set yet."
                        .to_owned())
                        .to_string(),
                }),
            }
        }
    }
    pub(crate) fn render_dot_11(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fixed((7.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((7.0) as f32)),
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[29]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: None,
                    width: None,
                    radius: Some([
                        (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                        (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                        (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                        (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Space {
                    width: Some(
                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                    ),
                    height: Some(
                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                    ),
                }),
            }
        }
    }
    pub(crate) fn render_group_label_18(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("YOUR ACCESS".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_gate_note_32(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (11.0) as f32,
                    right: (13.0) as f32,
                    bottom: (11.0) as f32,
                    left: (13.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[84]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[33]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((9.0) as f32).max(0.0).min(f32::MAX),
                        ((9.0) as f32).max(0.0).min(f32::MAX),
                        ((9.0) as f32).max(0.0).min(f32::MAX),
                        ((9.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:93", use_scope),
                                    width: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                                    ),
                                    height: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                                    ),
                                    padding: None,
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[34]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((3.0) as f32).max(0.0).min(f32::MAX),
                                            ((3.0) as f32).max(0.0).min(f32::MAX),
                                            ((3.0) as f32).max(0.0).min(f32::MAX),
                                            ((3.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                        width: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                        ),
                                        height: Some(
                                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                        ),
                                    }),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:92", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (4.0) as f32,
                                    right: (0.0) as f32,
                                    bottom: (0.0) as f32,
                                    left: (0.0) as f32,
                                }),
                                width: None,
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: Some(
                                            ::ducktape_view_guest::wire::LineHeight::Relative(
                                                ((1.45) as f32).max(f32::EPSILON).min(f32::MAX),
                                            ),
                                        ),
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:101", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[30]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align_x: None,
                                    content: ("Only a validator may open a membership proposal."
                                        .to_owned())
                                        .to_string(),
                                });
                            {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: Some(
                                                ::ducktape_view_guest::wire::LineHeight::Relative(
                                                    ((1.45) as f32).max(f32::EPSILON).min(f32::MAX),
                                                ),
                                            ),
                                            shaping: None,
                                            wrapping: None,
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:108", use_scope),
                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[70]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        align_x: None,
                                        content: ("Ask a validator to propose this node for the validator set — this device cannot open it."
                                            .to_owned())
                                            .to_string(),
                                    });
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:100", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((2.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:87", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((8.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Left),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_group_label_60(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("NODE".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_member_fact_row_61(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (9.0) as f32,
                    right: (12.0) as f32,
                    bottom: (9.0) as f32,
                    left: (12.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:249", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("public key".to_owned()).to_string(),
                        });
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(
                                    ::ducktape_view_guest::wire::Wrapping::WordOrGlyph,
                                ),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:260", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[13]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: (crate::host::keep_str(
                                    (!(self.facts.node_key).is_empty()),
                                    ::std::convert::AsRef::as_ref(&(self.facts.node_key)),
                                    ::std::convert::AsRef::as_ref(&("—")),
                                )
                                .to_owned())
                                .to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:244", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((10.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Left),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_member_fact_row_62(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (9.0) as f32,
                    right: (12.0) as f32,
                    bottom: (9.0) as f32,
                    left: (12.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                        ((8.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:249", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("data directory".to_owned()).to_string(),
                        });
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(
                                    ::ducktape_view_guest::wire::Wrapping::WordOrGlyph,
                                ),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:260", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[13]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: (crate::host::keep_str(
                                    (!(self.node_data_dir).is_empty()),
                                    ::std::convert::AsRef::as_ref(&(self.node_data_dir)),
                                    ::std::convert::AsRef::as_ref(&("—")),
                                )
                                .to_owned())
                                .to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:244", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((10.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Left),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_group_label_63(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("NETWORK".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_stat_card_64(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (11.0) as f32,
                    right: (13.0) as f32,
                    bottom: (11.0) as f32,
                    left: (13.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:213", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("HEIGHT".to_owned()).to_string(),
                        });
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:220", use_scope),
                                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::height_label_short(
                                            self.facts.node_height,
                                        )
                                        .to_owned())
                                        .to_string(),
                                });
                            ();
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:219", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((4.0) as f32),
                                padding: None,
                                width: None,
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:212", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((3.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_stat_card_65(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (11.0) as f32,
                    right: (13.0) as f32,
                    bottom: (11.0) as f32,
                    left: (13.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:213", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("CHECKPOINT".to_owned()).to_string(),
                        });
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:220", use_scope),
                                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::height_label_short(
                                            self.facts.node_checkpoint,
                                        )
                                        .to_owned())
                                        .to_string(),
                                });
                            ();
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:219", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((4.0) as f32),
                                padding: None,
                                width: None,
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:212", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((3.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_stat_card_66(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (11.0) as f32,
                    right: (13.0) as f32,
                    bottom: (11.0) as f32,
                    left: (13.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:213", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("LAST FINALIZED".to_owned()).to_string(),
                        });
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:220", use_scope),
                                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::relative_time(
                                            self.facts.node_last_finalized,
                                            self.wall_now,
                                        )
                                        .to_owned())
                                        .to_string(),
                                });
                            ();
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:219", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((4.0) as f32),
                                padding: None,
                                width: None,
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:212", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((3.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_stat_card_67(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (11.0) as f32,
                    right: (13.0) as f32,
                    bottom: (11.0) as f32,
                    left: (13.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                        ((10.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:213", use_scope),
                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[73]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("VALIDATORS REACHED".to_owned()).to_string(),
                        });
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:220", use_scope),
                                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::reading_pair(
                                            ::std::convert::AsRef::as_ref(
                                                &(self.facts.node_reachable_label),
                                            ),
                                            ::std::convert::AsRef::as_ref(
                                                &(self.facts.node_quorum_label),
                                            ),
                                        )
                                        .to_owned())
                                        .to_string(),
                                });
                            {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:227", use_scope),
                                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("of quorum".to_owned()).to_string(),
                                    });
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:219", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((4.0) as f32),
                                padding: None,
                                width: None,
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:212", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((3.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_key_value_row_68(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Node version".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("—".to_owned()).to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:49", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_key_value_row_69(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Node version".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (self.facts.node_version.to_owned()).to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:49", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_key_value_row_71(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Phase".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::reading_pair(
                                            ::std::convert::AsRef::as_ref(&(self.facts.sync_line)),
                                            ::std::convert::AsRef::as_ref(
                                                &(crate::host::relative_time(
                                                    self.facts.node_phase_since,
                                                    self.wall_now,
                                                )),
                                            ),
                                        )
                                        .to_owned())
                                        .to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:49", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_key_value_row_72(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Sync retries / failures, cumulative".to_owned())
                                        .to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::reading_pair(
                                            ::std::convert::AsRef::as_ref(
                                                &(crate::host::count_label(self.facts.node_sync_retries)),
                                            ),
                                            ::std::convert::AsRef::as_ref(
                                                &(crate::host::count_label(self.facts.node_sync_failures)),
                                            ),
                                        )
                                        .to_owned())
                                        .to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                if (!(self.facts.node_sync_last_error).is_empty()) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:49", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_key_value_row_73(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Last sync error".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (self.facts.node_sync_last_error.to_owned())
                                        .to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:49", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_key_value_row_74(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("App hash".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (self.facts.node_root_hash.to_owned()).to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_group_card_75(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn() -> Message + Clone + 'static,
        cb_1: impl Fn(String, String) -> Message + Clone + 'static,
        cb_2: impl Fn(String) -> Message + Clone + 'static,
        cb_3: impl Fn(String) -> Message + Clone + 'static,
        cb_4: impl Fn() -> Message + Clone + 'static,
        cb_5: impl Fn(NodeTab) -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(
                                    self
                                        .render_node_build_row_70(
                                            palette,
                                            format!("{}/NodeBuildRow@2281", use_scope),
                                        ),
                                );
                            children
                                .push(
                                    self
                                        .render_key_value_row_71(
                                            palette,
                                            format!("{}/KeyValueRow@2282", use_scope),
                                        ),
                                );
                            children
                                .push(
                                    self
                                        .render_key_value_row_72(
                                            palette,
                                            format!("{}/KeyValueRow@2291", use_scope),
                                        ),
                                );
                            if (!(self.facts.node_sync_last_error).is_empty()) {
                                children
                                    .push(
                                        self
                                            .render_key_value_row_73(
                                                palette,
                                                format!("{}/KeyValueRow@2300", use_scope),
                                            ),
                                    );
                            }
                            children
                                .push(
                                    self
                                        .render_key_value_row_74(
                                            palette,
                                            format!("{}/KeyValueRow@2305", use_scope),
                                        ),
                                );
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:416", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: None,
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:21", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: None,
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_group_label_76(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("PEERS".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_group_card_77(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn() -> Message + Clone + 'static,
        cb_1: impl Fn(String, String) -> Message + Clone + 'static,
        cb_2: impl Fn(String) -> Message + Clone + 'static,
        cb_3: impl Fn(String) -> Message + Clone + 'static,
        cb_4: impl Fn() -> Message + Clone + 'static,
        cb_5: impl Fn(NodeTab) -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            for (index, peer) in self.node_peers.iter().enumerate() {
                                let for_scope = format!(
                                    "{}/@for:2315({})", use_scope, index
                                );
                                children
                                    .push(::ducktape_view_guest::wire::Node::Container {
                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                            color: None,
                                            x: None,
                                            y: None,
                                            blur: None,
                                        },
                                        max_width: None,
                                        max_height: None,
                                        clip: false,
                                        key: format!("{}/@container:452", for_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (11.0) as f32,
                                            right: (15.0) as f32,
                                            bottom: (11.0) as f32,
                                            left: (15.0) as f32,
                                        }),
                                        align_x: None,
                                        align_y: None,
                                        background: (None)
                                            .map(::ducktape_view_guest::wire::Background::Color),
                                        border: None,
                                        snap: None,
                                        content: Box::new({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            if peer.live {
                                                children
                                                    .push(
                                                        self
                                                            .render_dot_11(palette, format!("{}/Dot@2327", for_scope)),
                                                    );
                                            }
                                            if (!peer.live) {
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:465", for_scope),
                                                        width: Some(
                                                            ::ducktape_view_guest::wire::Length::Fixed((7.0) as f32),
                                                        ),
                                                        height: Some(
                                                            ::ducktape_view_guest::wire::Length::Fixed((7.0) as f32),
                                                        ),
                                                        padding: None,
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (Some(palette.colors[98]))
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: None,
                                                            width: None,
                                                            radius: Some([
                                                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                                                ((3.5) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                        snap: None,
                                                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                                            width: Some(
                                                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                            ),
                                                            height: Some(
                                                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                            ),
                                                        }),
                                                    });
                                            }
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                        height: None,
                                                        align_y: None,
                                                        line_height: None,
                                                        shaping: None,
                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                        tracking: 0.0f32,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist Mono".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:472", for_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[4]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    align_x: None,
                                                    content: (peer.key.to_owned()).to_string(),
                                                });
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                        height: None,
                                                        align_y: None,
                                                        line_height: None,
                                                        shaping: None,
                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                        tracking: 0.0f32,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist Mono".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:479", for_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[5]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: (peer.role.to_owned()).to_string(),
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:457", for_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                spacing: Some((8.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        }),
                                    });
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:450", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: None,
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:21", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: None,
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
}
impl NodeView {
    pub(crate) fn render_log_timeline_frame_59(
        &self,
        palette: Palette,
        use_scope: String,
        cb_0: impl Fn() -> Message + Clone + 'static,
        cb_1: impl Fn(String, String) -> Message + Clone + 'static,
        cb_2: impl Fn(String) -> Message + Clone + 'static,
        cb_3: impl Fn(String) -> Message + Clone + 'static,
        cb_4: impl Fn() -> Message + Clone + 'static,
        cb_5: impl Fn(NodeTab) -> Message + Clone + 'static,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: 20.0,
                    right: 20.0,
                    bottom: 20.0,
                    left: 20.0,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[39]),
                    width: Some(1.0),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: Some(
                                            ::ducktape_view_guest::wire::LineHeight::Relative(1.35f32),
                                        ),
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:9", use_scope),
                                    size: Some(16.0f32),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Log ring".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: Some(
                                            ::ducktape_view_guest::wire::LineHeight::Relative(1.5f32),
                                        ),
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:10", use_scope),
                                    size: Some(12.5f32),
                                    color: Some(palette.colors[5]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Live node events retained in the in-memory ring."
                                        .to_owned())
                                        .to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:8", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((4.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push({
                                            let node_scope = format!("{}/log-filter", node_scope);
                                            ::ducktape_view_guest::wire::Node::Input {
                                                options: ::ducktape_view_guest::wire::InputOptions {
                                                    label: ("Filter logs".to_owned()).to_string(),
                                                    description: None,
                                                    disabled: false,
                                                    padding: Some(
                                                        ::ducktape_view_guest::wire::Edges::all((6.2) as f32),
                                                    ),
                                                    text_size: Some((13.0) as f32),
                                                    line_height: Some((1.2) as f32),
                                                    align: None,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: node_scope.clone(),
                                                placeholder: String::from("filter logs…".to_owned()),
                                                value: (self.node_log_filter).to_string(),
                                                on_input: ::ducktape_view_guest::slots::handler::<
                                                    String,
                                                    Message,
                                                >(
                                                    Box::new({
                                                        let route = {
                                                            let route_callback = (cb_3).clone();
                                                            move |value| (route_callback)(value)
                                                        };
                                                        move |sent: String| Some(route(sent))
                                                    }),
                                                ),
                                                on_submit: None,
                                                width: Some(
                                                    ::ducktape_view_guest::wire::Length::Fixed((200.0) as f32),
                                                ),
                                                secure: (false),
                                                style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                    utility: ::ducktape_view_guest::wire::InputFace {
                                                        background: Some(palette.colors[3]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(palette.colors[39]),
                                                            width: Some(1f32),
                                                            radius: Some([10f32; 4]),
                                                        }),
                                                        ..Default::default()
                                                    },
                                                    focus_border: Some(palette.colors[42]),
                                                    focused_hovered: None,
                                                    active: ::ducktape_view_guest::wire::InputFace {
                                                        icon: None,
                                                        background: Some(palette.colors[3]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: None,
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                        value: Some(palette.colors[4]),
                                                        placeholder: Some(palette.colors[72]),
                                                        selection: Some({
                                                            let mut color = palette.colors[4];
                                                            color.0[3] = 0.180000;
                                                            color
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                        icon: None,
                                                        background: Some(palette.colors[6]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(palette.colors[40]),
                                                            width: None,
                                                            radius: None,
                                                        }),
                                                        value: None,
                                                        placeholder: None,
                                                        selection: None,
                                                    }),
                                                    focused: None,
                                                    disabled: None,
                                                }),
                                            }
                                        });
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Space {
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                        });
                                    if self.admin {
                                        children
                                            .push({
                                                let node_scope = format!("{}/live-log-filter", node_scope);
                                                ::ducktape_view_guest::wire::Node::Input {
                                                    options: ::ducktape_view_guest::wire::InputOptions {
                                                        label: ("Live tracing filter".to_owned()).to_string(),
                                                        description: None,
                                                        disabled: false,
                                                        padding: Some(
                                                            ::ducktape_view_guest::wire::Edges::all((6.2) as f32),
                                                        ),
                                                        text_size: Some((13.0) as f32),
                                                        line_height: Some((1.2) as f32),
                                                        align: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: node_scope.clone(),
                                                    placeholder: String::from(
                                                        "info,ducktape::join=debug".to_owned(),
                                                    ),
                                                    value: (self.live_log_filter).to_string(),
                                                    on_input: ::ducktape_view_guest::slots::handler::<
                                                        String,
                                                        Message,
                                                    >(
                                                        Box::new({
                                                            let route = {
                                                                let route_callback = (cb_2).clone();
                                                                move |value| (route_callback)(value)
                                                            };
                                                            move |sent: String| Some(route(sent))
                                                        }),
                                                    ),
                                                    on_submit: Some(
                                                        ::ducktape_view_guest::slots::message((cb_0)()),
                                                    ),
                                                    width: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((260.0) as f32),
                                                    ),
                                                    secure: (false),
                                                    style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                        utility: ::ducktape_view_guest::wire::InputFace {
                                                            background: Some(palette.colors[3]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(palette.colors[39]),
                                                                width: Some(1f32),
                                                                radius: Some([10f32; 4]),
                                                            }),
                                                            ..Default::default()
                                                        },
                                                        focus_border: Some(palette.colors[42]),
                                                        focused_hovered: None,
                                                        active: ::ducktape_view_guest::wire::InputFace {
                                                            icon: None,
                                                            background: Some(palette.colors[3]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                radius: Some([
                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ]),
                                                            }),
                                                            value: Some(palette.colors[4]),
                                                            placeholder: Some(palette.colors[72]),
                                                            selection: Some({
                                                                let mut color = palette.colors[4];
                                                                color.0[3] = 0.180000;
                                                                color
                                                            }),
                                                        },
                                                        hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                            icon: None,
                                                            background: Some(palette.colors[6]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(palette.colors[40]),
                                                                width: None,
                                                                radius: None,
                                                            }),
                                                            value: None,
                                                            placeholder: None,
                                                            selection: None,
                                                        }),
                                                        focused: None,
                                                        disabled: None,
                                                    }),
                                                }
                                            });
                                    }
                                    if self.admin {
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:363", use_scope),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                    String::from("Retune"),
                                                ),
                                                label: None,
                                                on_press: if ((self.live_log_filter).is_empty()) {
                                                    None
                                                } else {
                                                    Some(::ducktape_view_guest::slots::message((cb_0)()))
                                                },
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(palette.colors[12]),
                                                            text: Some(palette.colors[13]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(palette.colors[40]),
                                                                width: Some(1.0),
                                                                radius: Some([9.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[6]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face::default(),
                                                    hovered: None,
                                                    pressed: None,
                                                    disabled: None,
                                                },
                                            });
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:330", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                        spacing: Some((9.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: Some(::ducktape_view_guest::wire::AlignX::Right),
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                            if (!(self.live_filter_note).is_empty()) {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: None,
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:369", use_scope),
                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[5]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (self.live_filter_note.to_owned()).to_string(),
                                    });
                            }
                            children
                                .push(
                                    self
                                        .render_log_console_58(
                                            palette,
                                            format!("{}/LogConsole@2234", use_scope),
                                        ),
                                );
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:329", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((9.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:7", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((12.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
}
impl NodeView {
    pub(crate) fn render_module_hash_8(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (arg_0).is_empty() {
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:724", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[13]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("—".to_owned()).to_string(),
                        });
                }
                if (!(arg_0).is_empty()) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:731", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[13]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.to_owned()).to_string(),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_module_hash_field_9(
        &self,
        palette: Palette,
        use_scope: String,
        arg_1: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: None,
                            shaping: None,
                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                            tracking: 0.0f32,
                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                    "Geist Mono".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:713", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("root".to_owned()).to_string(),
                    });
                children
                    .push(
                        self
                            .render_module_hash_8(
                                palette,
                                format!("{}/ModuleHash@1353", use_scope),
                                arg_1.to_owned(),
                            ),
                    );
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((5.0) as f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_module_hash_field_10(
        &self,
        palette: Palette,
        use_scope: String,
        arg_1: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: None,
                            shaping: None,
                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                            tracking: 0.0f32,
                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                    "Geist Mono".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:713", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("code".to_owned()).to_string(),
                    });
                children
                    .push(
                        self
                            .render_module_hash_8(
                                palette,
                                format!("{}/ModuleHash@1353", use_scope),
                                arg_1.to_owned(),
                            ),
                    );
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((5.0) as f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_module_swap_chip_12(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_0 {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:768", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (4.0) as f32,
                                right: (9.0) as f32,
                                bottom: (4.0) as f32,
                                left: (9.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[18]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[19]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist Mono".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:776", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[16]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("SWAP ARMED".to_owned()).to_string(),
                            }),
                        });
                }
                if (!arg_0) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:783", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (4.0) as f32,
                                right: (9.0) as f32,
                                bottom: (4.0) as f32,
                                left: (9.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[32]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[33]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist Mono".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:791", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[30]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("SWAP PENDING".to_owned()).to_string(),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_module_state_chip_13(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: bool,
        arg_1: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (!arg_0) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:743", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (4.0) as f32,
                                right: (9.0) as f32,
                                bottom: (4.0) as f32,
                                left: (9.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[27]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[28]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                children
                                    .push(
                                        self
                                            .render_dot_11(palette, format!("{}/Dot@1386", use_scope)),
                                    );
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:753", use_scope),
                                        size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[25]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("ACTIVE".to_owned()).to_string(),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:751", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                    spacing: Some((6.0) as f32),
                                    padding: None,
                                    width: None,
                                    height: None,
                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            }),
                        });
                }
                if arg_0 {
                    children
                        .push(
                            self
                                .render_module_swap_chip_12(
                                    palette,
                                    format!("{}/ModuleSwapChip@1394", use_scope),
                                    arg_1,
                                ),
                        );
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_module_hash_field_14(
        &self,
        palette: Palette,
        use_scope: String,
        arg_1: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: None,
                            shaping: None,
                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                            tracking: 0.0f32,
                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                    "Geist Mono".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:713", use_scope),
                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[73]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("target".to_owned()).to_string(),
                    });
                children
                    .push(
                        self
                            .render_module_hash_8(
                                palette,
                                format!("{}/ModuleHash@1353", use_scope),
                                arg_1.to_owned(),
                            ),
                    );
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((5.0) as f32),
                    padding: None,
                    width: None,
                    height: None,
                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_module_pending_plate_15(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::ModuleRow,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (11.0) as f32,
                    right: (13.0) as f32,
                    bottom: (11.0) as f32,
                    left: (13.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[90]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[19]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((9.0) as f32).max(0.0).min(f32::MAX),
                        ((9.0) as f32).max(0.0).min(f32::MAX),
                        ((9.0) as f32).max(0.0).min(f32::MAX),
                        ((9.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:821", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[16]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("PENDING SWAP".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(
                                    self
                                        .render_module_hash_field_14(
                                            palette,
                                            format!("{}/ModuleHashField@1462", use_scope),
                                            arg_0.pending_hash.to_owned(),
                                        ),
                                );
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:816", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((7.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Text {
                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                height: None,
                                                align_y: None,
                                                line_height: None,
                                                shaping: None,
                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                tracking: 0.0f32,
                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                        "Geist Mono".into(),
                                                    ),
                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:835", use_scope),
                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[73]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: ("ACTIVATES AT".to_owned()).to_string(),
                                        });
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Text {
                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                height: None,
                                                align_y: None,
                                                line_height: None,
                                                shaping: None,
                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                tracking: 0.0f32,
                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                        "Geist Mono".into(),
                                                    ),
                                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:841", use_scope),
                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[4]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: (crate::host::height_label_short(
                                                arg_0.activation_height,
                                            ))
                                                .to_string(),
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:834", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((3.0) as f32),
                                        padding: None,
                                        width: None,
                                        height: None,
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Text {
                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                height: None,
                                                align_y: None,
                                                line_height: None,
                                                shaping: None,
                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                tracking: 0.0f32,
                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                        "Geist Mono".into(),
                                                    ),
                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:848", use_scope),
                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[73]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: ("READY SIGNALS".to_owned()).to_string(),
                                        });
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Text {
                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                height: None,
                                                align_y: None,
                                                line_height: None,
                                                shaping: None,
                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                tracking: 0.0f32,
                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                        "Geist Mono".into(),
                                                    ),
                                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:854", use_scope),
                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[4]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: (arg_0.readiness).to_string(),
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:847", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((3.0) as f32),
                                        padding: None,
                                        width: None,
                                        height: None,
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:829", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((22.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Left),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:815", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((9.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_module_card_16(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::ModuleRow,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (13.0) as f32,
                    right: (15.0) as f32,
                    bottom: (13.0) as f32,
                    left: (15.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:669", use_scope),
                                    width: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((40.0) as f32),
                                    ),
                                    height: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((40.0) as f32),
                                    ),
                                    padding: None,
                                    align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                    align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((10.0) as f32).max(0.0).min(f32::MAX),
                                            ((10.0) as f32).max(0.0).min(f32::MAX),
                                            ((10.0) as f32).max(0.0).min(f32::MAX),
                                            ((10.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:677", use_scope),
                                        size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[70]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (crate::host::initial_of(
                                            ::std::convert::AsRef::as_ref(&(arg_0.id)),
                                        ))
                                            .to_string(),
                                    }),
                                });
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Text {
                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                height: None,
                                                align_y: None,
                                                line_height: None,
                                                shaping: None,
                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                tracking: 0.0f32,
                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                        "Geist".into(),
                                                    ),
                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: format!("{}/@text:684", use_scope),
                                            size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[4]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: (arg_0.id.to_owned()).to_string(),
                                        });
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                        height: None,
                                                        align_y: None,
                                                        line_height: None,
                                                        shaping: None,
                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                        tracking: 0.0f32,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist Mono".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:696", use_scope),
                                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[70]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: (arg_0.category.to_owned()).to_string(),
                                                });
                                            children
                                                .push(
                                                    self
                                                        .render_module_hash_field_9(
                                                            palette,
                                                            format!("{}/ModuleHashField@1336", use_scope),
                                                            arg_0.root.to_owned(),
                                                        ),
                                                );
                                            children
                                                .push(
                                                    self
                                                        .render_module_hash_field_10(
                                                            palette,
                                                            format!("{}/ModuleHashField@1337", use_scope),
                                                            arg_0.code_hash.to_owned(),
                                                        ),
                                                );
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:690", use_scope),
                                                wrap: Some(::ducktape_view_guest::wire::Wrap {
                                                    spacing: Some((3.0) as f32),
                                                    align: None,
                                                }),
                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                spacing: Some((8.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:683", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((3.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                            children
                                .push(
                                    self
                                        .render_module_state_chip_13(
                                            palette,
                                            format!("{}/ModuleStateChip@1338", use_scope),
                                            (!(arg_0.pending_hash).is_empty()),
                                            arg_0.ready,
                                        ),
                                );
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:664", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((11.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    if (!(arg_0.pending_hash).is_empty()) {
                        children
                            .push(
                                self
                                    .render_module_pending_plate_15(
                                        palette,
                                        format!("{}/ModulePendingPlate@1340", use_scope),
                                        arg_0.clone(),
                                    ),
                            );
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:663", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((11.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_modules_panel_17(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(
                                self
                                    .render_group_label_6(
                                        palette,
                                        format!("{}/GroupLabel@1247", use_scope),
                                    ),
                            );
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:614", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                padding: None,
                                align_x: None,
                                align_y: None,
                                background: (Some(palette.colors[60]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                    ),
                                    height: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                    ),
                                }),
                            });
                        children
                            .push({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:621", use_scope),
                                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[73]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (((self.module_rows).len() as i64)).to_string(),
                                    });
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:627", use_scope),
                                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[73]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("registered".to_owned()).to_string(),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:620", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                    spacing: Some((5.0) as f32),
                                    padding: None,
                                    width: None,
                                    height: None,
                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:608", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((12.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                if (self.module_rows).is_empty() {
                    children
                        .push(
                            self
                                .render_empty_plate_7(
                                    palette,
                                    format!("{}/EmptyPlate@1268", use_scope),
                                ),
                        );
                }
                if (!(self.module_rows).is_empty()) {
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            for (index, entry) in self.module_rows.iter().enumerate() {
                                let for_scope = format!(
                                    "{}/@for:1271({})", use_scope, index
                                );
                                children
                                    .push(
                                        self
                                            .render_module_card_16(
                                                palette,
                                                format!("{}/ModuleCard@1272", for_scope),
                                                entry.clone(),
                                            ),
                                    );
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:636", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((9.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: Some((13.0) as f32),
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_19(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:354", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                background: (Some(palette.colors[27]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: None,
                                    width: None,
                                    radius: Some([
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:362", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[25]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✓".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:368", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Sign quorum & finalize rounds".to_owned())
                                    .to_string(),
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:349", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_20(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:354", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                background: (Some(palette.colors[27]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: None,
                                    width: None,
                                    radius: Some([
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:362", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[25]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✓".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:368", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Invite members & assign roles".to_owned())
                                    .to_string(),
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:349", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_21(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:354", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                background: (Some(palette.colors[27]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: None,
                                    width: None,
                                    radius: Some([
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:362", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[25]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✓".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:368", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Install & remove modules".to_owned()).to_string(),
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:349", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_22(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:354", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                background: (Some(palette.colors[27]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: None,
                                    width: None,
                                    radius: Some([
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:362", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[25]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✓".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:368", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Edit network settings".to_owned()).to_string(),
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:349", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_23(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:354", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                background: (Some(palette.colors[27]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: None,
                                    width: None,
                                    radius: Some([
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:362", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[25]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✓".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:368", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Read & verify finality".to_owned()).to_string(),
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:349", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_24(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:354", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                background: (Some(palette.colors[27]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: None,
                                    width: None,
                                    radius: Some([
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:362", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[25]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✓".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:368", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Send · react · thread".to_owned()).to_string(),
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:349", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_25(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                {
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:379", use_scope),
                                    width: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                    ),
                                    height: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                    ),
                                    padding: None,
                                    align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                    align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((8.5) as f32).max(0.0).min(f32::MAX),
                                            ((8.5) as f32).max(0.0).min(f32::MAX),
                                            ((8.5) as f32).max(0.0).min(f32::MAX),
                                            ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:387", use_scope),
                                        size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[74]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("–".to_owned()).to_string(),
                                    }),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:393", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[74]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align_x: None,
                                    content: ("Propose modules & members".to_owned())
                                        .to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:374", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((8.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_26(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                {
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:379", use_scope),
                                    width: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                    ),
                                    height: Some(
                                        ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                    ),
                                    padding: None,
                                    align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                    align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((8.5) as f32).max(0.0).min(f32::MAX),
                                            ((8.5) as f32).max(0.0).min(f32::MAX),
                                            ((8.5) as f32).max(0.0).min(f32::MAX),
                                            ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:387", use_scope),
                                        size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[74]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("–".to_owned()).to_string(),
                                    }),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:393", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[74]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align_x: None,
                                    content: ("Sign quorum · finalize".to_owned()).to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:374", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((8.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_gated_chip_28(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (5.0) as f32,
                    right: (10.0) as f32,
                    bottom: (5.0) as f32,
                    left: (10.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[87]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[60]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(
                            self
                                .render_icon_27(palette, format!("{}/Icon@1046", use_scope)),
                        );
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:417", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Propose modules & members".to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:411", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((6.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_gated_chip_29(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (5.0) as f32,
                    right: (10.0) as f32,
                    bottom: (5.0) as f32,
                    left: (10.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[87]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[60]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(
                            self
                                .render_icon_27(palette, format!("{}/Icon@1046", use_scope)),
                        );
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:417", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Invite members".to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:411", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((6.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_gated_chip_30(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (5.0) as f32,
                    right: (10.0) as f32,
                    bottom: (5.0) as f32,
                    left: (10.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[87]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[60]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(
                            self
                                .render_icon_27(palette, format!("{}/Icon@1046", use_scope)),
                        );
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:417", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Change roles".to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:411", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((6.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_gated_chip_31(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (5.0) as f32,
                    right: (10.0) as f32,
                    bottom: (5.0) as f32,
                    left: (10.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[87]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[60]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(
                            self
                                .render_icon_27(palette, format!("{}/Icon@1046", use_scope)),
                        );
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:417", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Network settings".to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:411", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((6.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_capability_check_33(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:354", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                background: (Some(palette.colors[27]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: None,
                                    width: None,
                                    radius: Some([
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:362", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[25]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✓".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:368", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Read chat & threads".to_owned()).to_string(),
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:349", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_34(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:354", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                background: (Some(palette.colors[27]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: None,
                                    width: None,
                                    radius: Some([
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:362", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[25]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✓".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:368", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Read governance".to_owned()).to_string(),
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:349", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_capability_check_35(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:354", use_scope),
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((17.0) as f32),
                                ),
                                padding: None,
                                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                background: (Some(palette.colors[27]))
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: Some(::ducktape_view_guest::wire::Border {
                                    color: None,
                                    width: None,
                                    radius: Some([
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                        ((8.5) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:362", use_scope),
                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[25]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("✓".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:368", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[15]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                align_x: None,
                                content: ("Browse Forge".to_owned()).to_string(),
                            });
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:349", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((8.0) as f32),
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_gated_chip_36(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (5.0) as f32,
                    right: (10.0) as f32,
                    bottom: (5.0) as f32,
                    left: (10.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[87]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[60]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(
                            self
                                .render_icon_27(palette, format!("{}/Icon@1046", use_scope)),
                        );
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:417", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Propose".to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:411", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((6.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_gated_chip_37(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (5.0) as f32,
                    right: (10.0) as f32,
                    bottom: (5.0) as f32,
                    left: (10.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[87]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[60]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(
                            self
                                .render_icon_27(palette, format!("{}/Icon@1046", use_scope)),
                        );
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:417", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Forge contribute & merge".to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:411", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((6.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_gated_chip_38(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (5.0) as f32,
                    right: (10.0) as f32,
                    bottom: (5.0) as f32,
                    left: (10.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[87]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[60]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(
                            self
                                .render_icon_27(palette, format!("{}/Icon@1046", use_scope)),
                        );
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:417", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Sign quorum".to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:411", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((6.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_gated_chip_39(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (5.0) as f32,
                    right: (10.0) as f32,
                    bottom: (5.0) as f32,
                    left: (10.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[87]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[60]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                        ((7.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(
                            self
                                .render_icon_27(palette, format!("{}/Icon@1046", use_scope)),
                        );
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:417", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Invite".to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:411", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((6.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_node_access_card_40(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(
                        self
                            .render_group_label_18(
                                palette,
                                format!("{}/GroupLabel@705", use_scope),
                            ),
                    );
                if (self.tier == "validator") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:74", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (16.0) as f32,
                                right: (18.0) as f32,
                                bottom: (16.0) as f32,
                                left: (18.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[106]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[28]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                            height: None,
                                                            align_y: None,
                                                            line_height: None,
                                                            shaping: None,
                                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                            tracking: 0.0f32,
                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                    "Geist".into(),
                                                                ),
                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:92", use_scope),
                                                        size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[7]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("This node".to_owned()).to_string(),
                                                    });
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:98", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (2.0) as f32,
                                                            right: (7.0) as f32,
                                                            bottom: (2.0) as f32,
                                                            left: (7.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (Some(palette.colors[7]))
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: None,
                                                            width: None,
                                                            radius: Some([
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                        snap: None,
                                                        content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                height: None,
                                                                align_y: None,
                                                                line_height: None,
                                                                shaping: None,
                                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                tracking: 0.0f32,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist Mono".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: format!("{}/@text:104", use_scope),
                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[9]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("VALIDATOR · QUORUM SEAT".to_owned()).to_string(),
                                                        }),
                                                    });
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:87", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((7.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist Mono".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:110", use_scope),
                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[70]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("signs quorum · finalizes rounds · stores all history"
                                                    .to_owned())
                                                    .to_string(),
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:86", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((3.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_19(
                                                                palette,
                                                                format!("{}/CapabilityCheck@752", use_scope),
                                                            ),
                                                    );
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_20(
                                                                palette,
                                                                format!("{}/CapabilityCheck@753", use_scope),
                                                            ),
                                                    );
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:117", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((9.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_21(
                                                                palette,
                                                                format!("{}/CapabilityCheck@755", use_scope),
                                                            ),
                                                    );
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_22(
                                                                palette,
                                                                format!("{}/CapabilityCheck@756", use_scope),
                                                            ),
                                                    );
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:120", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((9.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:116", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((9.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Container {
                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:124", use_scope),
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: Some(
                                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                ),
                                                padding: None,
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[28]))
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                                    width: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                    ),
                                                    height: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                    ),
                                                }),
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: Some(
                                                        ::ducktape_view_guest::wire::LineHeight::Relative(
                                                            ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
                                                        ),
                                                    ),
                                                    shaping: None,
                                                    wrapping: None,
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:130", use_scope),
                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[70]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                align_x: None,
                                                content: ("A quorum seat is granted and revoked by quorum only — this device cannot change it."
                                                    .to_owned())
                                                    .to_string(),
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:123", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((13.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:85", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                    spacing: Some((14.0) as f32),
                                    padding: None,
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    align: None,
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            }),
                        });
                }
                if ((!(self.tier == "validator")) && (self.tier == "resident")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:137", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (16.0) as f32,
                                right: (18.0) as f32,
                                bottom: (16.0) as f32,
                                left: (18.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[3]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[63]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                            height: None,
                                                            align_y: None,
                                                            line_height: None,
                                                            shaping: None,
                                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                            tracking: 0.0f32,
                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                    "Geist".into(),
                                                                ),
                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:155", use_scope),
                                                        size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[7]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("This node".to_owned()).to_string(),
                                                    });
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:161", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (2.0) as f32,
                                                            right: (7.0) as f32,
                                                            bottom: (2.0) as f32,
                                                            left: (7.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (Some(palette.colors[3]))
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(palette.colors[40]),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                        snap: None,
                                                        content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                height: None,
                                                                align_y: None,
                                                                line_height: None,
                                                                shaping: None,
                                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                tracking: 0.0f32,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist Mono".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: format!("{}/@text:169", use_scope),
                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[5]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("RESIDENT · FULL NODE".to_owned()).to_string(),
                                                        }),
                                                    });
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:150", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((7.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist Mono".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:175", use_scope),
                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[70]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("full node · stores all history · cannot sign quorum"
                                                    .to_owned())
                                                    .to_string(),
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:149", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((3.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_23(
                                                                palette,
                                                                format!("{}/CapabilityCheck@817", use_scope),
                                                            ),
                                                    );
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_24(
                                                                palette,
                                                                format!("{}/CapabilityCheck@818", use_scope),
                                                            ),
                                                    );
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:182", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((9.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_25(
                                                                palette,
                                                                format!("{}/CapabilityCheck@824", use_scope),
                                                            ),
                                                    );
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_26(
                                                                palette,
                                                                format!("{}/CapabilityCheck@825", use_scope),
                                                            ),
                                                    );
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:185", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((9.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:181", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((9.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Container {
                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:193", use_scope),
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: Some(
                                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                ),
                                                padding: None,
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[60]))
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                                    width: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                    ),
                                                    height: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                    ),
                                                }),
                                            });
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                            height: None,
                                                            align_y: None,
                                                            line_height: None,
                                                            shaping: None,
                                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                            tracking: 0.0f32,
                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                    "Geist Mono".into(),
                                                                ),
                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:200", use_scope),
                                                        size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[30]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("VALIDATORS ONLY · QUORUM-GATED".to_owned())
                                                            .to_string(),
                                                    });
                                                children
                                                    .push({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        children
                                                            .push(
                                                                self
                                                                    .render_gated_chip_28(
                                                                        palette,
                                                                        format!("{}/GatedChip@845", use_scope),
                                                                    ),
                                                            );
                                                        children
                                                            .push(
                                                                self
                                                                    .render_gated_chip_29(
                                                                        palette,
                                                                        format!("{}/GatedChip@846", use_scope),
                                                                    ),
                                                            );
                                                        children
                                                            .push(
                                                                self
                                                                    .render_gated_chip_30(
                                                                        palette,
                                                                        format!("{}/GatedChip@847", use_scope),
                                                                    ),
                                                            );
                                                        children
                                                            .push(
                                                                self
                                                                    .render_gated_chip_31(
                                                                        palette,
                                                                        format!("{}/GatedChip@848", use_scope),
                                                                    ),
                                                            );
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:206", use_scope),
                                                            wrap: Some(::ducktape_view_guest::wire::Wrap {
                                                                spacing: Some((7.0) as f32),
                                                                align: None,
                                                            }),
                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                            spacing: Some((7.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            align: None,
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    });
                                                children
                                                    .push(
                                                        self
                                                            .render_gate_note_32(
                                                                palette,
                                                                format!("{}/GateNote@855", use_scope),
                                                            ),
                                                    );
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:199", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                    spacing: Some((9.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:192", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((13.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:148", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                    spacing: Some((14.0) as f32),
                                    padding: None,
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    align: None,
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            }),
                        });
                }
                if ((!((self.tier == "validator") || (self.tier == "resident")))
                    && (self.tier == "guest"))
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:226", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (16.0) as f32,
                                right: (18.0) as f32,
                                bottom: (16.0) as f32,
                                left: (18.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[84]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[33]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                            height: None,
                                                            align_y: None,
                                                            line_height: None,
                                                            shaping: None,
                                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                            tracking: 0.0f32,
                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                    "Geist".into(),
                                                                ),
                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:244", use_scope),
                                                        size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[7]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("This node".to_owned()).to_string(),
                                                    });
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:250", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (2.0) as f32,
                                                            right: (7.0) as f32,
                                                            bottom: (2.0) as f32,
                                                            left: (7.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (Some(palette.colors[32]))
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(palette.colors[33]),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                                ((5.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                        snap: None,
                                                        content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                height: None,
                                                                align_y: None,
                                                                line_height: None,
                                                                shaping: None,
                                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                tracking: 0.0f32,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist Mono".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: format!("{}/@text:258", use_scope),
                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[30]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("GUEST · LIGHT NODE".to_owned()).to_string(),
                                                        }),
                                                    });
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:239", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((7.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist Mono".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:264", use_scope),
                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[70]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("read-only · verifies finalized headers"
                                                    .to_owned())
                                                    .to_string(),
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:238", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((3.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_23(
                                                                palette,
                                                                format!("{}/CapabilityCheck@906", use_scope),
                                                            ),
                                                    );
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_33(
                                                                palette,
                                                                format!("{}/CapabilityCheck@907", use_scope),
                                                            ),
                                                    );
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:271", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((9.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_34(
                                                                palette,
                                                                format!("{}/CapabilityCheck@909", use_scope),
                                                            ),
                                                    );
                                                children
                                                    .push(
                                                        self
                                                            .render_capability_check_35(
                                                                palette,
                                                                format!("{}/CapabilityCheck@910", use_scope),
                                                            ),
                                                    );
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:274", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((9.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:270", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((9.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Container {
                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:278", use_scope),
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: Some(
                                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                ),
                                                padding: None,
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[33]))
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                                    width: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                    ),
                                                    height: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                    ),
                                                }),
                                            });
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                            height: None,
                                                            align_y: None,
                                                            line_height: None,
                                                            shaping: None,
                                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                            tracking: 0.0f32,
                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                    "Geist Mono".into(),
                                                                ),
                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:285", use_scope),
                                                        size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[30]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("GUEST · NO SIGNING, NO CONTRIBUTION".to_owned())
                                                            .to_string(),
                                                    });
                                                children
                                                    .push({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        children
                                                            .push(
                                                                self
                                                                    .render_gated_chip_36(
                                                                        palette,
                                                                        format!("{}/GatedChip@930", use_scope),
                                                                    ),
                                                            );
                                                        children
                                                            .push(
                                                                self
                                                                    .render_gated_chip_37(
                                                                        palette,
                                                                        format!("{}/GatedChip@931", use_scope),
                                                                    ),
                                                            );
                                                        children
                                                            .push(
                                                                self
                                                                    .render_gated_chip_38(
                                                                        palette,
                                                                        format!("{}/GatedChip@932", use_scope),
                                                                    ),
                                                            );
                                                        children
                                                            .push(
                                                                self
                                                                    .render_gated_chip_39(
                                                                        palette,
                                                                        format!("{}/GatedChip@933", use_scope),
                                                                    ),
                                                            );
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:291", use_scope),
                                                            wrap: Some(::ducktape_view_guest::wire::Wrap {
                                                                spacing: Some((7.0) as f32),
                                                                align: None,
                                                            }),
                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                            spacing: Some((7.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            align: None,
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    });
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:284", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                    spacing: Some((9.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:277", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((13.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:237", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                    spacing: Some((14.0) as f32),
                                    padding: None,
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    align: None,
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            }),
                        });
                }
                if (!(((self.tier == "validator") || (self.tier == "resident"))
                    || (self.tier == "guest")))
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:301", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (16.0) as f32,
                                right: (18.0) as f32,
                                bottom: (16.0) as f32,
                                left: (18.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[3]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[63]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                    ((13.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:318", use_scope),
                                                size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[7]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("This node".to_owned()).to_string(),
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Container {
                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:324", use_scope),
                                                width: None,
                                                height: None,
                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                    top: (2.0) as f32,
                                                    right: (7.0) as f32,
                                                    bottom: (2.0) as f32,
                                                    left: (7.0) as f32,
                                                }),
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[55]))
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: Some(::ducktape_view_guest::wire::Border {
                                                    color: None,
                                                    width: None,
                                                    radius: Some([
                                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                                    ]),
                                                }),
                                                snap: None,
                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                        height: None,
                                                        align_y: None,
                                                        line_height: None,
                                                        shaping: None,
                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                        tracking: 0.0f32,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist Mono".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:330", use_scope),
                                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[71]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("STANDING UNKNOWN".to_owned()).to_string(),
                                                }),
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:313", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                            spacing: Some((7.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    });
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: Some(
                                                ::ducktape_view_guest::wire::LineHeight::Relative(
                                                    ((1.5) as f32).max(f32::EPSILON).min(f32::MAX),
                                                ),
                                            ),
                                            shaping: None,
                                            wrapping: None,
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:336", use_scope),
                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[70]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        align_x: None,
                                        content: ("The valset roster has not answered, so this node's standing is not known yet. Nothing is claimed until it does."
                                            .to_owned())
                                            .to_string(),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:312", use_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                    spacing: Some((9.0) as f32),
                                    padding: None,
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    align: None,
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: Some((9.0) as f32),
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_head_41(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.tier == "validator") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:491", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(palette.colors[91]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:498", use_scope),
                                size: Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[69]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Validator".to_owned()).to_string(),
                            }),
                        });
                }
                if (!(self.tier == "validator")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:505", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:512", use_scope),
                                size: Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[69]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Validator".to_owned()).to_string(),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_head_42(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.tier == "resident") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:491", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(palette.colors[91]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:498", use_scope),
                                size: Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[69]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Full".to_owned()).to_string(),
                            }),
                        });
                }
                if (!(self.tier == "resident")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:505", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:512", use_scope),
                                size: Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[69]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Full".to_owned()).to_string(),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_head_43(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.tier == "guest") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:491", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(palette.colors[91]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:498", use_scope),
                                size: Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[69]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Light".to_owned()).to_string(),
                            }),
                        });
                }
                if (!(self.tier == "guest")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:505", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:512", use_scope),
                                size: Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[69]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Light".to_owned()).to_string(),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_tick_44(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: None,
                            shaping: None,
                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                            tracking: 0.0f32,
                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                    "Geist".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:568", use_scope),
                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[25]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("✓".to_owned()).to_string(),
                    });
                ();
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_cell_45(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.tier == "validator") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:547", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(palette.colors[86]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_44(
                                        palette,
                                        format!("{}/MatrixTick@1188", use_scope),
                                    ),
                            ),
                        });
                }
                if (!(self.tier == "validator")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:556", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_44(
                                        palette,
                                        format!("{}/MatrixTick@1197", use_scope),
                                    ),
                            ),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_cell_46(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.tier == "resident") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:547", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(palette.colors[86]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_44(
                                        palette,
                                        format!("{}/MatrixTick@1188", use_scope),
                                    ),
                            ),
                        });
                }
                if (!(self.tier == "resident")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:556", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_44(
                                        palette,
                                        format!("{}/MatrixTick@1197", use_scope),
                                    ),
                            ),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_cell_47(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.tier == "guest") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:547", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(palette.colors[86]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_44(
                                        palette,
                                        format!("{}/MatrixTick@1188", use_scope),
                                    ),
                            ),
                        });
                }
                if (!(self.tier == "guest")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:556", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_44(
                                        palette,
                                        format!("{}/MatrixTick@1197", use_scope),
                                    ),
                            ),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_row_48(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:521", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(
                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                        ),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[55]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                        }),
                    });
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:528", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (11.0) as f32,
                                    right: (14.0) as f32,
                                    bottom: (11.0) as f32,
                                    left: (14.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:535", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Read & verify finality".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(
                                self
                                    .render_matrix_cell_45(
                                        palette,
                                        format!("{}/MatrixCell@1170", use_scope),
                                    ),
                            );
                        children
                            .push(
                                self
                                    .render_matrix_cell_46(
                                        palette,
                                        format!("{}/MatrixCell@1171", use_scope),
                                    ),
                            );
                        children
                            .push(
                                self
                                    .render_matrix_cell_47(
                                        palette,
                                        format!("{}/MatrixCell@1172", use_scope),
                                    ),
                            );
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:527", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: None,
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_tick_49(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:575", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[98]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("−".to_owned()).to_string(),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_cell_50(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.tier == "guest") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:547", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(palette.colors[86]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_49(
                                        palette,
                                        format!("{}/MatrixTick@1188", use_scope),
                                    ),
                            ),
                        });
                }
                if (!(self.tier == "guest")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:556", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_49(
                                        palette,
                                        format!("{}/MatrixTick@1197", use_scope),
                                    ),
                            ),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_row_51(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:521", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(
                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                        ),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[55]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                        }),
                    });
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:528", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (11.0) as f32,
                                    right: (14.0) as f32,
                                    bottom: (11.0) as f32,
                                    left: (14.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:535", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Send · react · thread".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(
                                self
                                    .render_matrix_cell_45(
                                        palette,
                                        format!("{}/MatrixCell@1170", use_scope),
                                    ),
                            );
                        children
                            .push(
                                self
                                    .render_matrix_cell_46(
                                        palette,
                                        format!("{}/MatrixCell@1171", use_scope),
                                    ),
                            );
                        children
                            .push(
                                self
                                    .render_matrix_cell_50(
                                        palette,
                                        format!("{}/MatrixCell@1172", use_scope),
                                    ),
                            );
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:527", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: None,
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_cell_52(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.tier == "resident") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:547", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(palette.colors[86]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_49(
                                        palette,
                                        format!("{}/MatrixTick@1188", use_scope),
                                    ),
                            ),
                        });
                }
                if (!(self.tier == "resident")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:556", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((92.0) as f32),
                            ),
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (11.0) as f32,
                                right: (0.0) as f32,
                                bottom: (11.0) as f32,
                                left: (0.0) as f32,
                            }),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(
                                self
                                    .render_matrix_tick_49(
                                        palette,
                                        format!("{}/MatrixTick@1197", use_scope),
                                    ),
                            ),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_row_53(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:521", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(
                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                        ),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[55]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                        }),
                    });
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:528", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (11.0) as f32,
                                    right: (14.0) as f32,
                                    bottom: (11.0) as f32,
                                    left: (14.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:535", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Propose modules & members".to_owned())
                                        .to_string(),
                                }),
                            });
                        children
                            .push(
                                self
                                    .render_matrix_cell_45(
                                        palette,
                                        format!("{}/MatrixCell@1170", use_scope),
                                    ),
                            );
                        children
                            .push(
                                self
                                    .render_matrix_cell_52(
                                        palette,
                                        format!("{}/MatrixCell@1171", use_scope),
                                    ),
                            );
                        children
                            .push(
                                self
                                    .render_matrix_cell_50(
                                        palette,
                                        format!("{}/MatrixCell@1172", use_scope),
                                    ),
                            );
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:527", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: None,
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_matrix_row_54(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:521", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(
                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                        ),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[55]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new(::ducktape_view_guest::wire::Node::Space {
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                        }),
                    });
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        children
                            .push(::ducktape_view_guest::wire::Node::Container {
                                shadow: ::ducktape_view_guest::wire::Shadow {
                                    color: None,
                                    x: None,
                                    y: None,
                                    blur: None,
                                },
                                max_width: None,
                                max_height: None,
                                clip: false,
                                key: format!("{}/@container:528", use_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (11.0) as f32,
                                    right: (14.0) as f32,
                                    bottom: (11.0) as f32,
                                    left: (14.0) as f32,
                                }),
                                align_x: None,
                                align_y: None,
                                background: (None)
                                    .map(::ducktape_view_guest::wire::Background::Color),
                                border: None,
                                snap: None,
                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:535", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Sign quorum · finalize".to_owned()).to_string(),
                                }),
                            });
                        children
                            .push(
                                self
                                    .render_matrix_cell_45(
                                        palette,
                                        format!("{}/MatrixCell@1170", use_scope),
                                    ),
                            );
                        children
                            .push(
                                self
                                    .render_matrix_cell_52(
                                        palette,
                                        format!("{}/MatrixCell@1171", use_scope),
                                    ),
                            );
                        children
                            .push(
                                self
                                    .render_matrix_cell_50(
                                        palette,
                                        format!("{}/MatrixCell@1172", use_scope),
                                    ),
                            );
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:527", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: None,
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_permission_matrix_55(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: Some(((640.0) as f32).max(0.0).min(f32::MAX)),
                        max_height: None,
                        clip: true,
                        key: format!("{}/@container:436", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[3]))
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: Some(::ducktape_view_guest::wire::Border {
                            color: Some(palette.colors[63]),
                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                            radius: Some([
                                ((12.0) as f32).max(0.0).min(f32::MAX),
                                ((12.0) as f32).max(0.0).min(f32::MAX),
                                ((12.0) as f32).max(0.0).min(f32::MAX),
                                ((12.0) as f32).max(0.0).min(f32::MAX),
                            ]),
                        }),
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:446", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: None,
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[87]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Container {
                                                shadow: ::ducktape_view_guest::wire::Shadow {
                                                    color: None,
                                                    x: None,
                                                    y: None,
                                                    blur: None,
                                                },
                                                max_width: None,
                                                max_height: None,
                                                clip: false,
                                                key: format!("{}/@container:448", use_scope),
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                    top: (10.0) as f32,
                                                    right: (14.0) as f32,
                                                    bottom: (10.0) as f32,
                                                    left: (14.0) as f32,
                                                }),
                                                align_x: None,
                                                align_y: None,
                                                background: (None)
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: None,
                                                snap: None,
                                                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                        height: None,
                                                        align_y: None,
                                                        line_height: None,
                                                        shaping: None,
                                                        wrapping: None,
                                                        tracking: 0.0f32,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    },
                                                    key: format!("{}/@text:455", use_scope),
                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[70]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("capability".to_owned()).to_string(),
                                                }),
                                            });
                                        children
                                            .push(
                                                self
                                                    .render_matrix_head_41(
                                                        palette,
                                                        format!("{}/MatrixHead@1090", use_scope),
                                                    ),
                                            );
                                        children
                                            .push(
                                                self
                                                    .render_matrix_head_42(
                                                        palette,
                                                        format!("{}/MatrixHead@1091", use_scope),
                                                    ),
                                            );
                                        children
                                            .push(
                                                self
                                                    .render_matrix_head_43(
                                                        palette,
                                                        format!("{}/MatrixHead@1092", use_scope),
                                                    ),
                                            );
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:447", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                            spacing: None,
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    }),
                                });
                            children
                                .push(
                                    self
                                        .render_matrix_row_48(
                                            palette,
                                            format!("{}/MatrixRow@1093", use_scope),
                                        ),
                                );
                            children
                                .push(
                                    self
                                        .render_matrix_row_51(
                                            palette,
                                            format!("{}/MatrixRow@1100", use_scope),
                                        ),
                                );
                            children
                                .push(
                                    self
                                        .render_matrix_row_53(
                                            palette,
                                            format!("{}/MatrixRow@1107", use_scope),
                                        ),
                                );
                            children
                                .push(
                                    self
                                        .render_matrix_row_54(
                                            palette,
                                            format!("{}/MatrixRow@1114", use_scope),
                                        ),
                                );
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:445", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: None,
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: Some((13.0) as f32),
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_log_level_56(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (arg_0 == "ERROR") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:937", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[20]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((48.0) as f32),
                            ),
                            align_x: None,
                            content: (arg_0.to_owned()).to_string(),
                        });
                }
                if (arg_0 == "WARN") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:945", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[30]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((48.0) as f32),
                            ),
                            align_x: None,
                            content: (arg_0.to_owned()).to_string(),
                        });
                }
                if ((arg_0 != "ERROR") && (arg_0 != "WARN")) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist Mono".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:953", use_scope),
                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[5]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((48.0) as f32),
                            ),
                            align_x: None,
                            content: (arg_0.to_owned()).to_string(),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_log_line_57(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::LogRow,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: None,
                            shaping: None,
                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                            tracking: 0.0f32,
                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                    "Geist Mono".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:916", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: Some(
                            ::ducktape_view_guest::wire::Length::Fixed((170.0) as f32),
                        ),
                        align_x: None,
                        content: (arg_0.time.to_owned()).to_string(),
                    });
                children
                    .push(
                        self
                            .render_log_level_56(
                                palette,
                                format!("{}/LogLevel@1557", use_scope),
                                arg_0.level.to_owned(),
                            ),
                    );
                children
                    .push(::ducktape_view_guest::wire::Node::Text {
                        options: ::ducktape_view_guest::wire::TextOptions {
                            height: None,
                            align_y: None,
                            line_height: None,
                            shaping: None,
                            wrapping: None,
                            tracking: 0.0f32,
                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                    "Geist Mono".into(),
                                ),
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:924", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        align_x: None,
                        content: (arg_0.message.to_owned()).to_string(),
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((6.0) as f32),
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: Some(::ducktape_view_guest::wire::AlignX::Left),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_log_console_58(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((420.0) as f32)),
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (13.0) as f32,
                    right: (13.0) as f32,
                    bottom: (13.0) as f32,
                    left: (13.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[55]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[60]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:884", use_scope),
                                    size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[73]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("NODE LOG".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:891", use_scope),
                                    size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[5]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::count_label(
                                        ((crate::host::visible_log(
                                            ::std::convert::AsRef::as_ref(&(self.log_lines)),
                                            ::std::convert::AsRef::as_ref(&(self.node_log_filter)),
                                        ))
                                            .len() as i64),
                                    ))
                                        .to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:883", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((8.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    if (!(crate::host::log_note(
                        ((self.log_lines).len() as i64),
                        ((crate::host::visible_log(
                            ::std::convert::AsRef::as_ref(&(self.log_lines)),
                            ::std::convert::AsRef::as_ref(&(self.node_log_filter)),
                        ))
                            .len() as i64),
                    ))
                        .is_empty())
                    {
                        children
                            .push(::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist Mono".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:898", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[5]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (crate::host::log_note(
                                        ((self.log_lines).len() as i64),
                                        ((crate::host::visible_log(
                                            ::std::convert::AsRef::as_ref(&(self.log_lines)),
                                            ::std::convert::AsRef::as_ref(&(self.node_log_filter)),
                                        ))
                                            .len() as i64),
                                    )
                                    .to_owned())
                                    .to_string(),
                            });
                    }
                    children
                        .push({
                            let node_scope = format!("{}/node-log-lines", node_scope);
                            ::ducktape_view_guest::wire::Node::Scroll {
                                on_scroll: None,
                                virtual_rows: false,
                                key: node_scope.clone(),
                                direction: ::ducktape_view_guest::wire::ScrollDirection::Vertical,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                bar_hidden: false,
                                bar_width: None,
                                bar_margin: None,
                                scroller_width: None,
                                bar_spacing: None,
                                anchor_x: ::ducktape_view_guest::wire::ScrollAnchor::Start,
                                anchor_y: ::ducktape_view_guest::wire::ScrollAnchor::End,
                                auto_scroll: (true),
                                background: None,
                                border: None,
                                content: Box::new({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    for (index, line) in crate::host::visible_log(
                                            ::std::convert::AsRef::as_ref(&(self.log_lines)),
                                            ::std::convert::AsRef::as_ref(&(self.node_log_filter)),
                                        )
                                        .iter()
                                        .enumerate()
                                    {
                                        let for_scope = format!(
                                            "{}/@for:1541({})", use_scope, index
                                        );
                                        children
                                            .push(
                                                self
                                                    .render_log_line_57(
                                                        palette,
                                                        format!("{}/LogLine@1542", for_scope),
                                                        line.clone(),
                                                    ),
                                            );
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:906", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((1.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                }),
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:882", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((8.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_node_build_row_70(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.facts.node_version).is_empty() {
                    children
                        .push(
                            self
                                .render_key_value_row_68(
                                    palette,
                                    format!("{}/KeyValueRow@673", use_scope),
                                ),
                        );
                }
                if (!(self.facts.node_version).is_empty()) {
                    children
                        .push(
                            self
                                .render_key_value_row_69(
                                    palette,
                                    format!("{}/KeyValueRow@679", use_scope),
                                ),
                        );
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
}
ducktape_view_guest::export_app!(
    NodeView, "Node",
    "This node: coherent status, standing, peers, logs and the code registry.", ["node"]
);
