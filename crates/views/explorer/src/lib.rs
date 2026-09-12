//! The block explorer as a module-owned view on the kernel contract.
//!
//! The kernel pushes session facts only (`explorer.props`: connected, dark,
//! and the live head and sync line the app already holds). The block window
//! is this view's own — `rpc.blocks`, re-read on every `rpc.live` hit for the
//! `block` plane — and so is the workspace search, which fans out over
//! `rpc.query` and `rpc.view`. A clipboard copy is the one act that leaves as
//! an intent: the OS door is the kernel's. The endpoint, the key and the
//! password never cross.
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
pub struct ExplorerView {
    pub(crate) active_palette: AppTheme,
    pub(crate) connected: bool,
    pub(crate) loading: bool,
    pub(crate) blocks: Vec<crate::host::ExplorerBlock>,
    pub(crate) ops: Vec<crate::host::ExplorerOp>,
    pub(crate) head: i64,
    pub(crate) sync_line: String,
    pub(crate) ledger_serial: i64,
    pub(crate) hits: Vec<crate::host::ExplorerHit>,
    pub(crate) kinds: Vec<crate::host::KindCount>,
    pub(crate) partial: String,
    pub(crate) searching: bool,
    pub(crate) sent_query: String,
    pub(crate) search_serial: i64,
    pub(crate) query: String,
    pub(crate) kind: String,
    pub(crate) selected: i64,
    pub(crate) viewport_width: f64,
    pub(crate) ledger_width: f64,
    pub(crate) host_error: String,
    pub(crate) sent: bool,
}
impl ::std::fmt::Debug for ExplorerView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("ExplorerView")
    }
}
#[derive(Clone)]
pub enum Message {
    SessionArrived(crate::host::SessionItem),
    LedgerArrived(crate::host::LedgerItem),
    SearchArrived(crate::host::SearchItem),
    Refresh,
    CopyToClipboard(String, String),
    SearchSubmit,
    ClearExplorerSearch,
    PickExplorerKind(String),
    SelectExplorerBlock(i64),
    LedgerResized(f64, f64),
    ViewportChanged(f64, f64),
    BindQuery(String),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl ExplorerView {
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
impl ExplorerView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            connected: false,
            loading: false,
            blocks: Vec::new(),
            ops: Vec::new(),
            head: 0,
            sync_line: "".to_owned(),
            ledger_serial: 0,
            hits: Vec::new(),
            kinds: Vec::new(),
            partial: "".to_owned(),
            searching: false,
            sent_query: "".to_owned(),
            search_serial: 0,
            query: "".to_owned(),
            kind: "all".to_owned(),
            selected: 0,
            viewport_width: 1280.0,
            ledger_width: 340.0,
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
        connected: bool,
        loading: bool,
        blocks: Vec<crate::host::ExplorerBlock>,
        ops: Vec<crate::host::ExplorerOp>,
        head: i64,
        sync_line: String,
        ledger_serial: i64,
        hits: Vec<crate::host::ExplorerHit>,
        kinds: Vec<crate::host::KindCount>,
        partial: String,
        searching: bool,
        sent_query: String,
        search_serial: i64,
        query: String,
        kind: String,
        selected: i64,
        viewport_width: f64,
        ledger_width: f64,
        host_error: String,
        sent: bool,
    ) -> Self {
        Self {
            active_palette: active_palette,
            connected: connected,
            loading: loading,
            blocks: blocks,
            ops: ops,
            head: head,
            sync_line: sync_line,
            ledger_serial: ledger_serial,
            hits: hits,
            kinds: kinds,
            partial: partial,
            searching: searching,
            sent_query: sent_query,
            search_serial: search_serial,
            query: query,
            kind: kind,
            selected: selected,
            viewport_width: viewport_width,
            ledger_width: ledger_width,
            host_error: host_error,
            sent: sent,
        }
    }
    pub(crate) const SNAPSHOT_SCHEMA: &'static str = "2d39c39a6f939b7c1759e6f118706ed6fb7b0dee7d160d3f2bab3a3d45e380e9";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("ExplorerView"),
                fields: vec![
                    (String::from("active_palette"), match & self.active_palette {
                    AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("AppTheme"), fields : vec![(String::from("app"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    AppTheme::AppDark =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AppTheme"), fields : vec![(String::from("app_dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected))), (String::from("loading"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .loading))), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ExplorerBlock"), fields :
                    ::std::vec![(String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .height))), (String::from("hash"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).hash))), (String::from("commit"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).commit))), (String::from("op_count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .op_count)))] }).collect())), (String::from("ops"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.ops).iter()
                    .map(| item | ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("ExplorerOp"), fields :
                    ::std::vec![(String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .height))), (String::from("proposer"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).proposer))), (String::from("target"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).target))), (String::from("disposition"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).disposition))), (String::from("op_hash"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).op_hash))), (String::from("payload"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).payload))), (String::from("trace"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).trace)))] }).collect())), (String::from("head"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self.head))),
                    (String::from("sync_line"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.sync_line))), (String::from("ledger_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .ledger_serial))), (String::from("hits"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.hits).iter()
                    .map(| item | ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("ExplorerHit"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("code"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).code))), (String::from("title"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).title))), (String::from("snippet"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).snippet))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).meta))), (String::from("target"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).target)))] }).collect())), (String::from("kinds"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.kinds)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("KindCount"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("label"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).label))), (String::from("count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).count)))]
                    }).collect())), (String::from("partial"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.partial))), (String::from("searching"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .searching))), (String::from("sent_query"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.sent_query))), (String::from("search_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .search_serial))), (String::from("query"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.query))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.kind))), (String::from("selected"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .selected))), (String::from("viewport_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .viewport_width))), (String::from("ledger_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .ledger_width))), (String::from("host_error"),
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
            if name != "ExplorerView" || fields.len() != 21 {
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
            if name != "connected" {
                return None;
            }
            let connected: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "loading" {
                return None;
            }
            let loading: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "blocks" {
                return None;
            }
            let blocks: Vec<crate::host::ExplorerBlock> = (match value {
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
                            if name != "ExplorerBlock" || fields.len() != 4 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "height" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "hash" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "commit" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "op_count" {
                                return None;
                            }
                            Some(crate::host::ExplorerBlock {
                                height: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                hash: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                commit: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                op_count: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
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
            if name != "ops" {
                return None;
            }
            let ops: Vec<crate::host::ExplorerOp> = (match value {
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
                            if name != "ExplorerOp" || fields.len() != 7 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "height" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "proposer" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "target" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "disposition" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "op_hash" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "payload" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "trace" {
                                return None;
                            }
                            Some(crate::host::ExplorerOp {
                                height: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                proposer: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                target: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                disposition: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                op_hash: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                payload: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                trace: (match field_6 {
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
            if name != "head" {
                return None;
            }
            let head: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sync_line" {
                return None;
            }
            let sync_line: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "ledger_serial" {
                return None;
            }
            let ledger_serial: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "hits" {
                return None;
            }
            let hits: Vec<crate::host::ExplorerHit> = (match value {
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
                            if name != "ExplorerHit" || fields.len() != 6 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "kind" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "code" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "title" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "snippet" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "meta" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "target" {
                                return None;
                            }
                            Some(crate::host::ExplorerHit {
                                kind: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                code: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                title: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                snippet: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                meta: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                target: (match field_5 {
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
            if name != "kinds" {
                return None;
            }
            let kinds: Vec<crate::host::KindCount> = (match value {
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
                            if name != "KindCount" || fields.len() != 3 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "kind" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "label" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "count" {
                                return None;
                            }
                            Some(crate::host::KindCount {
                                kind: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                label: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                count: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
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
            if name != "partial" {
                return None;
            }
            let partial: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "searching" {
                return None;
            }
            let searching: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sent_query" {
                return None;
            }
            let sent_query: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "search_serial" {
                return None;
            }
            let search_serial: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "query" {
                return None;
            }
            let query: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "kind" {
                return None;
            }
            let kind: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "selected" {
                return None;
            }
            let selected: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "viewport_width" {
                return None;
            }
            let viewport_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "ledger_width" {
                return None;
            }
            let ledger_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
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
                    connected,
                    loading,
                    blocks,
                    ops,
                    head,
                    sync_line,
                    ledger_serial,
                    hits,
                    kinds,
                    partial,
                    searching,
                    sent_query,
                    search_serial,
                    query,
                    kind,
                    selected,
                    viewport_width,
                    ledger_width,
                    host_error,
                    sent,
                ),
            )
        })())
            .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl ExplorerView {
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::ledger(self.ledger_serial)
                        .map(move |value| Message::LedgerArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (!(self.sent_query).is_empty())) {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::workspace_search(
                            self.sent_query.to_owned(),
                            self.search_serial,
                        )
                        .map(move |value| Message::SearchArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
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
                let (app, _) = ExplorerView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
impl ExplorerView {
    pub(crate) fn render_explorer_block_face_11(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::ExplorerBlock,
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
                        key: format!("{}/@text:703", use_scope),
                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (arg_0.height).to_string(),
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
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:713", use_scope),
                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        align_x: None,
                        content: (crate::host::hex(
                            ::std::convert::AsRef::as_ref(&(arg_0.hash)),
                        ))
                            .to_string(),
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
                        key: format!("{}/@text:725", use_scope),
                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: (crate::host::plural(
                            arg_0.op_count,
                            ::std::convert::AsRef::as_ref(&("op")),
                            ::std::convert::AsRef::as_ref(&("ops")),
                        ))
                            .to_string(),
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((8.0) as f32),
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_explorer_block_row_12(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::ExplorerBlock,
        arg_1: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_1 {
                    children
                        .push(::ducktape_view_guest::wire::Node::Button {
                            checked: Some(arg_1),
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:671", use_scope),
                            content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                Box::new(
                                    self
                                        .render_explorer_block_face_11(
                                            palette,
                                            format!("{}/ExplorerBlockFace@1702", use_scope),
                                            arg_0.clone(),
                                        ),
                                ),
                            ),
                            label: Some(String::from("Inspect block".to_owned())),
                            on_press: Some(
                                ::ducktape_view_guest::slots::message(
                                    Message::SelectExplorerBlock(arg_0.height),
                                ),
                            ),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: Some(
                                ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
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
                                    background: Some(palette.colors[91]),
                                    text: Some(palette.colors[4]),
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
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                },
                                hovered: Some(::ducktape_view_guest::wire::Face {
                                    background: Some(palette.colors[57]),
                                    text: Some(palette.colors[4]),
                                    border: None,
                                }),
                                pressed: Some(::ducktape_view_guest::wire::Face {
                                    background: Some(palette.colors[14]),
                                    text: None,
                                    border: None,
                                }),
                                disabled: None,
                            },
                        });
                }
                if (!arg_1) {
                    children
                        .push(::ducktape_view_guest::wire::Node::Button {
                            checked: Some(arg_1),
                            expanded: None,
                            description: None,
                            key: format!("{}/@button:683", use_scope),
                            content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                Box::new(
                                    self
                                        .render_explorer_block_face_11(
                                            palette,
                                            format!("{}/ExplorerBlockFace@1714", use_scope),
                                            arg_0.clone(),
                                        ),
                                ),
                            ),
                            label: Some(String::from("Inspect block".to_owned())),
                            on_press: Some(
                                ::ducktape_view_guest::slots::message(
                                    Message::SelectExplorerBlock(arg_0.height),
                                ),
                            ),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: Some(
                                ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
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
                                    text: Some(palette.colors[4]),
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
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                },
                                hovered: Some(::ducktape_view_guest::wire::Face {
                                    background: Some(palette.colors[57]),
                                    text: Some(palette.colors[4]),
                                    border: None,
                                }),
                                pressed: Some(::ducktape_view_guest::wire::Face {
                                    background: Some(palette.colors[14]),
                                    text: None,
                                    border: None,
                                }),
                                disabled: None,
                            },
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
    pub(crate) fn render_digest_row_14(
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
                        key: format!("{}/@text:645", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("block".to_owned()).to_string(),
                    });
                children
                    .push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:651", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                            Box::new(::ducktape_view_guest::wire::Node::Text {
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
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:656", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[5]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (crate::host::hex(
                                    ::std::convert::AsRef::as_ref(&(arg_1)),
                                ))
                                    .to_string(),
                            }),
                        ),
                        label: Some(String::from("Copy block hash".to_owned())),
                        on_press: Some(
                            ::ducktape_view_guest::slots::message(
                                Message::CopyToClipboard(
                                    arg_1.to_owned(),
                                    "Block hash copied".to_owned(),
                                ),
                            ),
                        ),
                        width: None,
                        height: None,
                        padding: Some(
                            ::ducktape_view_guest::wire::Edges::all((2.0) as f32),
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
                                text: Some(palette.colors[4]),
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
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[57]),
                                text: Some(palette.colors[4]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[14]),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
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
            }
        }
    }
    pub(crate) fn render_digest_row_15(
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
                        key: format!("{}/@text:645", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("commit".to_owned()).to_string(),
                    });
                children
                    .push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:651", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                            Box::new(::ducktape_view_guest::wire::Node::Text {
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
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:656", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[5]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (crate::host::hex(
                                    ::std::convert::AsRef::as_ref(&(arg_1)),
                                ))
                                    .to_string(),
                            }),
                        ),
                        label: Some(String::from("Copy commit hash".to_owned())),
                        on_press: Some(
                            ::ducktape_view_guest::slots::message(
                                Message::CopyToClipboard(
                                    arg_1.to_owned(),
                                    "Commit hash copied".to_owned(),
                                ),
                            ),
                        ),
                        width: None,
                        height: None,
                        padding: Some(
                            ::ducktape_view_guest::wire::Edges::all((2.0) as f32),
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
                                text: Some(palette.colors[4]),
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
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[57]),
                                text: Some(palette.colors[4]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[14]),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
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
            }
        }
    }
    pub(crate) fn render_digest_row_21(
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
                        key: format!("{}/@text:645", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("hash".to_owned()).to_string(),
                    });
                children
                    .push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:651", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                            Box::new(::ducktape_view_guest::wire::Node::Text {
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
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:656", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[5]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (crate::host::hex(
                                    ::std::convert::AsRef::as_ref(&(arg_1)),
                                ))
                                    .to_string(),
                            }),
                        ),
                        label: Some(String::from("Copy op hash".to_owned())),
                        on_press: Some(
                            ::ducktape_view_guest::slots::message(
                                Message::CopyToClipboard(
                                    arg_1.to_owned(),
                                    "Op hash copied".to_owned(),
                                ),
                            ),
                        ),
                        width: None,
                        height: None,
                        padding: Some(
                            ::ducktape_view_guest::wire::Edges::all((2.0) as f32),
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
                                text: Some(palette.colors[4]),
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
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[57]),
                                text: Some(palette.colors[4]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[14]),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
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
            }
        }
    }
    pub(crate) fn render_digest_row_22(
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
                        key: format!("{}/@text:645", use_scope),
                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[5]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("by".to_owned()).to_string(),
                    });
                children
                    .push(::ducktape_view_guest::wire::Node::Button {
                        checked: None,
                        expanded: None,
                        description: None,
                        key: format!("{}/@button:651", use_scope),
                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                            Box::new(::ducktape_view_guest::wire::Node::Text {
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
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: format!("{}/@text:656", use_scope),
                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[5]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (crate::host::hex(
                                    ::std::convert::AsRef::as_ref(&(arg_1)),
                                ))
                                    .to_string(),
                            }),
                        ),
                        label: Some(String::from("Copy proposer".to_owned())),
                        on_press: Some(
                            ::ducktape_view_guest::slots::message(
                                Message::CopyToClipboard(
                                    arg_1.to_owned(),
                                    "Proposer copied".to_owned(),
                                ),
                            ),
                        ),
                        width: None,
                        height: None,
                        padding: Some(
                            ::ducktape_view_guest::wire::Edges::all((2.0) as f32),
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
                                text: Some(palette.colors[4]),
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
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                        ((5.0) as f32).max(0.0).min(f32::MAX),
                                    ]),
                                }),
                            },
                            hovered: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[57]),
                                text: Some(palette.colors[4]),
                                border: None,
                            }),
                            pressed: Some(::ducktape_view_guest::wire::Face {
                                background: Some(palette.colors[14]),
                                text: None,
                                border: None,
                            }),
                            disabled: None,
                        },
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
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
            }
        }
    }
}
impl ExplorerView {
    #[allow(clippy::assign_op_pattern)]
    pub(crate) fn update(
        &mut self,
        message: Message,
    ) -> ::ducktape_view_guest::Task<Message> {
        match message {
            Message::SessionArrived(item) => self.on_session_arrived(item),
            Message::LedgerArrived(item) => self.on_ledger_arrived(item),
            Message::SearchArrived(item) => self.on_search_arrived(item),
            Message::Refresh => self.on_refresh(),
            Message::CopyToClipboard(text, label) => {
                self.on_copy_to_clipboard(text, label)
            }
            Message::SearchSubmit => self.on_search_submit(),
            Message::ClearExplorerSearch => self.on_clear_explorer_search(),
            Message::PickExplorerKind(next) => self.on_pick_explorer_kind(next),
            Message::SelectExplorerBlock(height) => self.on_select_explorer_block(height),
            Message::LedgerResized(dx, _dy) => self.on_ledger_resized(dx, _dy),
            Message::ViewportChanged(width, _height) => {
                self.on_viewport_changed(width, _height)
            }
            Message::BindQuery(value) => self.on_bind_query(value),
        }
    }
    fn on_session_arrived(
        &mut self,
        item: crate::host::SessionItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.host_error = item.error.to_owned();
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            let next = item.next.clone();
            {
                self.head = next.head;
            }
            {
                self.sync_line = next.sync_line.to_owned();
            }
            {
                self.ledger_serial = crate::host::connection_serial_after(
                    self.connected,
                    next.connected,
                    self.ledger_serial,
                );
            }
            {
                self.loading = crate::host::loading_after(
                    self.connected,
                    next.connected,
                    self.loading,
                );
            }
            {
                self.connected = next.connected;
            }
            {
                self.active_palette = AppTheme::App;
            }
            if (!next.dark) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.active_palette = AppTheme::AppDark;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_ledger_arrived(
        &mut self,
        item: crate::host::LedgerItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.loading = false;
            }
            {
                self.host_error = item.error.to_owned();
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.blocks = item.blocks.clone();
            }
            {
                self.ops = item.ops.clone();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_search_arrived(
        &mut self,
        item: crate::host::SearchItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.searching = false;
            }
            {
                self.host_error = item.error.to_owned();
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.hits = item.hits.clone();
            }
            {
                self.kinds = item.kinds.clone();
            }
            {
                self.partial = item.partial.to_owned();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_refresh(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((!self.connected) || self.loading) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.loading = true;
            }
            {
                self.ledger_serial = (self.ledger_serial + 1);
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
                self.sent = crate::host::copy(
                    ::std::convert::AsRef::as_ref(&(text)),
                    ::std::convert::AsRef::as_ref(&(label)),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_search_submit(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            let blocked = (((!self.connected) || self.searching)
                || ((self.query).trim().to_owned()).is_empty());
            if blocked {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.kind = "all".to_owned();
            }
            {
                self.hits = Vec::new();
            }
            {
                self.kinds = Vec::new();
            }
            {
                self.partial = "".to_owned();
            }
            {
                self.searching = true;
            }
            {
                self.search_serial = (self.search_serial + 1);
            }
            {
                self.sent_query = (self.query).trim().to_owned();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_clear_explorer_search(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.query = "".to_owned();
            }
            {
                self.kind = "all".to_owned();
            }
            {
                self.hits = Vec::new();
            }
            {
                self.kinds = Vec::new();
            }
            {
                self.partial = "".to_owned();
            }
            {
                self.searching = false;
            }
            {
                self.sent_query = "".to_owned();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_pick_explorer_kind(
        &mut self,
        next: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.kind = next.to_owned();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_select_explorer_block(
        &mut self,
        height: i64,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.selected = height;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_ledger_resized(
        &mut self,
        dx: f64,
        _dy: f64,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.ledger_width = crate::host::ledger_width_after_delta(
                    self.ledger_width,
                    dx,
                    self.viewport_width,
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_viewport_changed(
        &mut self,
        width: f64,
        _height: f64,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.viewport_width = width;
            }
            {
                self.ledger_width = crate::host::ledger_width_after_delta(
                    self.ledger_width,
                    0.0,
                    width,
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_query(&mut self, value: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.query = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
}
impl ExplorerView {
    pub(crate) fn view(&self) -> ::ducktape_view_guest::wire::Node {
        let palette = self.palette();
        {
            let node_scope = format!("{}/root", "ExplorerView");
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
                    children
                        .push(::ducktape_view_guest::wire::Node::Sensor {
                            key: format!("{}/@sensor:164", node_scope),
                            reset: None,
                            on_show: Some(
                                ::ducktape_view_guest::slots::handler::<
                                    (f32, f32),
                                    Message,
                                >(
                                    Box::new({
                                        let route = move |size: (f64, f64)| Message::ViewportChanged(
                                            size.0,
                                            size.1,
                                        );
                                        move |sent: (f32, f32)| Some(
                                            route((f64::from(sent.0), f64::from(sent.1))),
                                        )
                                    }),
                                ),
                            ),
                            on_resize: Some(
                                ::ducktape_view_guest::slots::handler::<
                                    (f32, f32),
                                    Message,
                                >(
                                    Box::new({
                                        let route = move |size: (f64, f64)| Message::ViewportChanged(
                                            size.0,
                                            size.1,
                                        );
                                        move |sent: (f32, f32)| Some(
                                            route((f64::from(sent.0), f64::from(sent.1))),
                                        )
                                    }),
                                ),
                            ),
                            on_hide: None,
                            anticipate: None,
                            delay: None,
                            child: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((0.0) as f32),
                                ),
                            }),
                        });
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(
                                    self
                                        .render_screen_title_0(
                                            palette,
                                            format!("{}/ScreenTitle@1208", node_scope),
                                        ),
                                );
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
                                            key: format!("{}/@text:201", node_scope),
                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[5]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: (crate::host::height_label(self.head)).to_string(),
                                        });
                                    if (!(self.sync_line).is_empty()) {
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
                                                key: format!("{}/@text:203", node_scope),
                                                size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[5]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: (self.sync_line.to_owned()).to_string(),
                                            });
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:196", node_scope),
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
                            if (!(self.host_error).is_empty()) {
                                children
                                    .push({
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
                                    });
                            }
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: Some(((860.0) as f32).max(0.0).min(f32::MAX)),
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:211", node_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: None,
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
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
                                                key: format!("{}/@container:217", node_scope),
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                    top: (2.0) as f32,
                                                    right: (14.0) as f32,
                                                    bottom: (2.0) as f32,
                                                    left: (14.0) as f32,
                                                }),
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[3]))
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: Some(::ducktape_view_guest::wire::Border {
                                                    color: Some(palette.colors[7]),
                                                    width: Some(((1.5) as f32).max(0.0).min(f32::MAX)),
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
                                                        .push(
                                                            self
                                                                .render_icon_1(palette, format!("{}/Icon@1257", node_scope)),
                                                        );
                                                    children
                                                        .push({
                                                            let node_scope = format!("{}/explorer-search", node_scope);
                                                            ::ducktape_view_guest::wire::Node::Input {
                                                                options: ::ducktape_view_guest::wire::InputOptions {
                                                                    label: ("Search this workspace".to_owned()).to_string(),
                                                                    description: None,
                                                                    disabled: ((!self.connected) || self.searching),
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
                                                                    "Search messages, pages, issues, files, runs…".to_owned(),
                                                                ),
                                                                value: (self.query).to_string(),
                                                                on_input: ::ducktape_view_guest::slots::handler::<
                                                                    String,
                                                                    Message,
                                                                >(
                                                                    Box::new({
                                                                        let route = Message::BindQuery as fn(String) -> Message;
                                                                        move |sent: String| Some(route(sent))
                                                                    }),
                                                                ),
                                                                on_submit: Some(
                                                                    ::ducktape_view_guest::slots::message(Message::SearchSubmit),
                                                                ),
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
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
                                                                        background: Some(
                                                                            ::ducktape_view_guest::wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(
                                                                                ::ducktape_view_guest::wire::Rgba([
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.000000,
                                                                                ]),
                                                                            ),
                                                                            width: Some(((0.0) as f32).max(0.0).min(f32::MAX)),
                                                                            radius: Some([
                                                                                ((0.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((0.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((0.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((0.0) as f32).max(0.0).min(f32::MAX),
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
                                                                        background: Some(
                                                                            ::ducktape_view_guest::wire::Rgba([
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.0 / 255.0,
                                                                                0.000000,
                                                                            ]),
                                                                        ),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(
                                                                                ::ducktape_view_guest::wire::Rgba([
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.0 / 255.0,
                                                                                    0.000000,
                                                                                ]),
                                                                            ),
                                                                            width: None,
                                                                            radius: None,
                                                                        }),
                                                                        value: None,
                                                                        placeholder: None,
                                                                        selection: None,
                                                                    }),
                                                                    focused: None,
                                                                    disabled: Some(::ducktape_view_guest::wire::InputFace {
                                                                        icon: None,
                                                                        background: None,
                                                                        border: None,
                                                                        value: Some(palette.colors[5]),
                                                                        placeholder: None,
                                                                        selection: None,
                                                                    }),
                                                                }),
                                                            }
                                                        });
                                                    if (!((self.query).trim().to_owned()).is_empty()) {
                                                        children
                                                            .push({
                                                                let node_scope = format!("{}/explorer-clear", node_scope);
                                                                ::ducktape_view_guest::wire::Node::Button {
                                                                    checked: None,
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
                                                                            key: format!("{}/@container:260", node_scope),
                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                            padding: None,
                                                                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
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
                                                                                key: format!("{}/@text:266", node_scope),
                                                                                size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[5]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("×".to_owned()).to_string(),
                                                                            }),
                                                                        }),
                                                                    ),
                                                                    label: Some(
                                                                        String::from("Clear workspace search".to_owned()),
                                                                    ),
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::ClearExplorerSearch,
                                                                        ),
                                                                    ),
                                                                    width: Some(
                                                                        ::ducktape_view_guest::wire::Length::Fixed((22.0) as f32),
                                                                    ),
                                                                    height: Some(
                                                                        ::ducktape_view_guest::wire::Length::Fixed((22.0) as f32),
                                                                    ),
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
                                                                                    radius: Some([7.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[14]),
                                                                            pressed_background: Some(palette.colors[39]),
                                                                            disabled_background: None,
                                                                            disabled_text: None,
                                                                            disabled_opacity: Some(0.5f32),
                                                                            focus_ring: Some(palette.colors[42]),
                                                                            text_size: Some(13.5f32),
                                                                            line_height: None,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
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
                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                    ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                ]),
                                                                            }),
                                                                        },
                                                                        hovered: Some(::ducktape_view_guest::wire::Face {
                                                                            background: Some(palette.colors[55]),
                                                                            text: Some(palette.colors[4]),
                                                                            border: None,
                                                                        }),
                                                                        pressed: Some(::ducktape_view_guest::wire::Face {
                                                                            background: Some(palette.colors[56]),
                                                                            text: Some(palette.colors[4]),
                                                                            border: None,
                                                                        }),
                                                                        disabled: None,
                                                                    },
                                                                }
                                                            });
                                                    }
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:228", node_scope),
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
                                        if self.searching {
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
                                                    key: format!("{}/@text:275", node_scope),
                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[70]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("Searching…".to_owned()).to_string(),
                                                });
                                        }
                                        if self.loading {
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
                                                    key: format!("{}/@text:281", node_scope),
                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[70]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("Loading…".to_owned()).to_string(),
                                                });
                                        }
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:286", node_scope),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                    String::from("Refresh"),
                                                ),
                                                label: Some(String::from("Refresh".to_owned())),
                                                on_press: if (self.loading) {
                                                    None
                                                } else {
                                                    Some(
                                                            ::ducktape_view_guest::slots::message(Message::Refresh),
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
                                                            background: Some(palette.colors[3]),
                                                            text: Some(palette.colors[15]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(palette.colors[39]),
                                                                width: Some(1.0),
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[6]),
                                                        pressed_background: Some(palette.colors[14]),
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
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:212", node_scope),
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
                            if (self.connected && (!(self.partial).is_empty())) {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Container {
                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                            color: None,
                                            x: None,
                                            y: None,
                                            blur: None,
                                        },
                                        max_width: Some(((860.0) as f32).max(0.0).min(f32::MAX)),
                                        max_height: None,
                                        clip: false,
                                        key: format!("{}/@container:303", node_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        padding: None,
                                        align_x: None,
                                        align_y: None,
                                        background: (None)
                                            .map(::ducktape_view_guest::wire::Background::Color),
                                        border: None,
                                        snap: None,
                                        content: Box::new({
                                            let node_scope = format!("{}/explorer-partial", node_scope);
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
                                                    top: (8.0) as f32,
                                                    right: (12.0) as f32,
                                                    bottom: (8.0) as f32,
                                                    left: (12.0) as f32,
                                                }),
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[32]))
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
                                                    key: format!("{}/@text:315", node_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[30]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    align_x: None,
                                                    content: (self.partial.to_owned()).to_string(),
                                                }),
                                            }
                                        }),
                                    });
                            }
                            if (self.connected && (!(self.kinds).is_empty())) {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Container {
                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                            color: None,
                                            x: None,
                                            y: None,
                                            blur: None,
                                        },
                                        max_width: Some(((860.0) as f32).max(0.0).min(f32::MAX)),
                                        max_height: None,
                                        clip: false,
                                        key: format!("{}/@container:330", node_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        padding: None,
                                        align_x: None,
                                        align_y: None,
                                        background: (None)
                                            .map(::ducktape_view_guest::wire::Background::Color),
                                        border: None,
                                        snap: None,
                                        content: Box::new({
                                            let mut items = Vec::new();
                                            let flex_child: ::ducktape_view_guest::wire::Node = ::ducktape_view_guest::wire::Node::Button {
                                                checked: Some((self.kind == "all")),
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:338", node_scope),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                    Box::new(
                                                        self
                                                            .render_filter_chip_2(
                                                                palette,
                                                                format!("{}/FilterChip@1368", node_scope),
                                                            ),
                                                    ),
                                                ),
                                                label: Some(String::from("Show every result".to_owned())),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::PickExplorerKind("all".to_owned()),
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
                                                        text: Some(palette.colors[4]),
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
                                            };
                                            items
                                                .push((
                                                    ::ducktape_view_guest::wire::FlexItem::default(),
                                                    flex_child,
                                                ));
                                            for (index, kind_count) in self.kinds.iter().enumerate() {
                                                let for_scope = format!(
                                                    "{}/@for:1376({})", node_scope, index
                                                );
                                                let flex_child: ::ducktape_view_guest::wire::Node = ::ducktape_view_guest::wire::Node::Button {
                                                    checked: Some((self.kind == kind_count.kind)),
                                                    expanded: None,
                                                    description: Some(
                                                        String::from(kind_count.label.to_owned()),
                                                    ),
                                                    key: format!("{}/@button:353", for_scope),
                                                    content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                        Box::new(
                                                            self
                                                                .render_filter_chip_3(
                                                                    palette,
                                                                    format!("{}/FilterChip@1384", for_scope),
                                                                    kind_count.label.to_owned(),
                                                                    kind_count.count,
                                                                    (self.kind == kind_count.kind),
                                                                ),
                                                        ),
                                                    ),
                                                    label: Some(
                                                        String::from("Filter results by kind".to_owned()),
                                                    ),
                                                    on_press: Some(
                                                        ::ducktape_view_guest::slots::message(
                                                            Message::PickExplorerKind(kind_count.kind.to_owned()),
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
                                                            text: Some(palette.colors[4]),
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
                                                };
                                                items
                                                    .push((
                                                        ::ducktape_view_guest::wire::FlexItem::default(),
                                                        flex_child,
                                                    ));
                                            }
                                            let (items, children) = items.into_iter().unzip();
                                            ::ducktape_view_guest::wire::Node::Flex {
                                                key: format!("{}/@layout:331", node_scope),
                                                items,
                                                children,
                                                background: None,
                                                border: None,
                                                layout: ::ducktape_view_guest::wire::FlexLayout {
                                                    direction: ::ducktape_view_guest::wire::FlexDirection::Row,
                                                    wrap: ::ducktape_view_guest::wire::FlexWrap::Wrap,
                                                    justify: None,
                                                    items: Some(
                                                        ::ducktape_view_guest::wire::FlexItemAlignment::Start,
                                                    ),
                                                    content: None,
                                                    row_gap: Some((7.0) as f32),
                                                    column_gap: Some((7.0) as f32),
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
                                        }),
                                    });
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:166", node_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((16.0) as f32),
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (22.0) as f32,
                                    right: (24.0) as f32,
                                    bottom: (0.0) as f32,
                                    left: (24.0) as f32,
                                }),
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
                            if (self.connected && (!(self.hits).is_empty())) {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Scroll {
                                        on_scroll: None,
                                        virtual_rows: false,
                                        key: format!("{}/@layout:384", node_scope),
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
                                        content: Box::new(::ducktape_view_guest::wire::Node::Container {
                                            shadow: ::ducktape_view_guest::wire::Shadow {
                                                color: None,
                                                x: None,
                                                y: None,
                                                blur: None,
                                            },
                                            max_width: Some(((860.0) as f32).max(0.0).min(f32::MAX)),
                                            max_height: None,
                                            clip: false,
                                            key: format!("{}/@container:389", node_scope),
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            padding: None,
                                            align_x: None,
                                            align_y: None,
                                            background: (None)
                                                .map(::ducktape_view_guest::wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                for (index, hit) in self.hits.iter().enumerate() {
                                                    let for_scope = format!(
                                                        "{}/@for:1415({})", node_scope, index
                                                    );
                                                    if ((self.kind == "all") || (hit.kind == self.kind)) {
                                                        children
                                                            .push(
                                                                self
                                                                    .render_explorer_card_6(
                                                                        palette,
                                                                        format!("{}/ExplorerCard@1417", for_scope),
                                                                        hit.clone(),
                                                                    ),
                                                            );
                                                    }
                                                }
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:390", node_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                    spacing: Some((8.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            }),
                                        }),
                                    });
                            }
                            if (self.connected && (!(self.hits).is_empty())) {
                                for (index, kind_count) in self.kinds.iter().enumerate() {
                                    let for_scope = format!(
                                        "{}/@for:1423({})", node_scope, index
                                    );
                                    if ((self.kind == kind_count.kind)
                                        && (kind_count.count <= 0))
                                    {
                                        children
                                            .push(
                                                self
                                                    .render_empty_plate_7(
                                                        palette,
                                                        format!("{}/EmptyPlate@1425", for_scope),
                                                    ),
                                            );
                                    }
                                }
                            }
                            if (((self.connected && (self.hits).is_empty())
                                && (self.partial).is_empty())
                                && crate::host::search_answer_stands(
                                    ::std::convert::AsRef::as_ref(&(self.sent_query)),
                                    ::std::convert::AsRef::as_ref(&(self.query)),
                                    self.searching,
                                ))
                            {
                                children
                                    .push({
                                        let node_scope = format!(
                                            "{}/explorer-nothing-matched", node_scope
                                        );
                                        self.render_empty_state_8(palette, node_scope.clone())
                                    });
                            }
                            if (!self.connected) {
                                children
                                    .push(
                                        self
                                            .render_empty_state_9(
                                                palette,
                                                format!("{}/EmptyState@1442", node_scope),
                                            ),
                                    );
                            }
                            if (((((self.connected && (self.hits).is_empty())
                                && (self.blocks).is_empty()) && (!self.loading))
                                && ((self.query).trim().to_owned()).is_empty())
                                && (self.host_error).is_empty())
                            {
                                children
                                    .push(
                                        self
                                            .render_empty_state_10(
                                                palette,
                                                format!("{}/EmptyState@1455", node_scope),
                                            ),
                                    );
                            }
                            if ((self.connected && (self.hits).is_empty())
                                && (!(self.blocks).is_empty()))
                            {
                                children
                                    .push({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push({
                                                let node_scope = format!("{}/ledger-pane", node_scope);
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
                                                    width: Some(
                                                        ::ducktape_view_guest::wire::Length::Fixed(
                                                            (self.ledger_width) as f32,
                                                        ),
                                                    ),
                                                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (6.0) as f32,
                                                        right: (6.0) as f32,
                                                        bottom: (6.0) as f32,
                                                        left: (6.0) as f32,
                                                    }),
                                                    align_x: None,
                                                    align_y: None,
                                                    background: (Some(palette.colors[6]))
                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: Some({
                                                            let mut color = palette.colors[4];
                                                            color.0[3] = 0.100000;
                                                            color
                                                        }),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((10.0) as f32).max(0.0).min(f32::MAX),
                                                            ((10.0) as f32).max(0.0).min(f32::MAX),
                                                            ((10.0) as f32).max(0.0).min(f32::MAX),
                                                            ((10.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                    snap: None,
                                                    content: Box::new(::ducktape_view_guest::wire::Node::Scroll {
                                                        on_scroll: None,
                                                        virtual_rows: false,
                                                        key: format!("{}/@layout:452", node_scope),
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
                                                            for (index, block) in self.blocks.iter().enumerate() {
                                                                let for_scope = format!(
                                                                    "{}/@for:1489({})", node_scope, index
                                                                );
                                                                children
                                                                    .push(
                                                                        self
                                                                            .render_explorer_block_row_12(
                                                                                palette,
                                                                                format!("{}/ExplorerBlockRow@1490", for_scope),
                                                                                block.clone(),
                                                                                (block.height == self.selected),
                                                                            ),
                                                                    );
                                                            }
                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                max_width: None,
                                                                clip: false,
                                                                key: format!("{}/@layout:460", node_scope),
                                                                wrap: None,
                                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                spacing: Some((1.0) as f32),
                                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                                    top: (0.0) as f32,
                                                                    right: (10.0) as f32,
                                                                    bottom: (0.0) as f32,
                                                                    left: (0.0) as f32,
                                                                }),
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                height: None,
                                                                align: None,
                                                                background: None,
                                                                border: None,
                                                                children: children,
                                                            }
                                                        }),
                                                    }),
                                                }
                                            });
                                        children
                                            .push({
                                                let node_scope = format!("{}/ledger-resize", node_scope);
                                                ::ducktape_view_guest::wire::Node::ResizeHandle {
                                                    key: node_scope.clone(),
                                                    on_press: None,
                                                    on_release: None,
                                                    on_drag: Some(
                                                        ::ducktape_view_guest::slots::handler::<
                                                            (f64, f64),
                                                            Message,
                                                        >(
                                                            Box::new({
                                                                let route = move |delta: (f64, f64)| Message::LedgerResized(
                                                                    delta.0,
                                                                    delta.1,
                                                                );
                                                                move |sent: (f64, f64)| Some(route(sent))
                                                            }),
                                                        ),
                                                    ),
                                                    cursor: Some(
                                                        ::ducktape_view_guest::wire::mouse::Cursor::ResizingHorizontally,
                                                    ),
                                                    content: Box::new({
                                                        let node_scope = format!("{}/ledger-divider", node_scope);
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
                                                            width: Some(
                                                                ::ducktape_view_guest::wire::Length::Fixed((10.0) as f32),
                                                            ),
                                                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            padding: None,
                                                            align_x: None,
                                                            align_y: None,
                                                            background: (None)
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
                                                        }
                                                    }),
                                                }
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
                                                key: format!("{}/@container:472", node_scope),
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                    top: (8.0) as f32,
                                                    right: (8.0) as f32,
                                                    bottom: (8.0) as f32,
                                                    left: (8.0) as f32,
                                                }),
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[6]))
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: Some(::ducktape_view_guest::wire::Border {
                                                    color: Some({
                                                        let mut color = palette.colors[4];
                                                        color.0[3] = 0.100000;
                                                        color
                                                    }),
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
                                                    if (self.selected <= 0) {
                                                        children
                                                            .push(
                                                                self
                                                                    .render_empty_state_13(
                                                                        palette,
                                                                        format!("{}/EmptyState@1507", node_scope),
                                                                    ),
                                                            );
                                                    }
                                                    if (self.selected > 0) {
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Scroll {
                                                                on_scroll: None,
                                                                virtual_rows: false,
                                                                key: format!("{}/@layout:488", node_scope),
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
                                                                    for (index, block) in self.blocks.iter().enumerate() {
                                                                        let for_scope = format!(
                                                                            "{}/@for:1523({})", node_scope, index
                                                                        );
                                                                        if (block.height == self.selected) {
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
                                                                                    key: format!("{}/@container:501", for_scope),
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    height: None,
                                                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                        top: (8.0) as f32,
                                                                                        right: (8.0) as f32,
                                                                                        bottom: (8.0) as f32,
                                                                                        left: (8.0) as f32,
                                                                                    }),
                                                                                    align_x: None,
                                                                                    align_y: None,
                                                                                    background: (Some(palette.colors[3]))
                                                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: Some({
                                                                                            let mut color = palette.colors[4];
                                                                                            color.0[3] = 0.100000;
                                                                                            color
                                                                                        }),
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
                                                                                            .push(
                                                                                                self
                                                                                                    .render_digest_row_14(
                                                                                                        palette,
                                                                                                        format!("{}/DigestRow@1534", for_scope),
                                                                                                        block.hash.to_owned(),
                                                                                                    ),
                                                                                            );
                                                                                        children
                                                                                            .push(
                                                                                                self
                                                                                                    .render_digest_row_15(
                                                                                                        palette,
                                                                                                        format!("{}/DigestRow@1542", for_scope),
                                                                                                        block.commit.to_owned(),
                                                                                                    ),
                                                                                            );
                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                            max_width: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@layout:509", for_scope),
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
                                                                                });
                                                                        }
                                                                    }
                                                                    for (index, op) in crate::host::explorer_ops_at(
                                                                            ::std::convert::AsRef::as_ref(&(self.ops)),
                                                                            self.selected,
                                                                        )
                                                                        .iter()
                                                                        .enumerate()
                                                                    {
                                                                        let for_scope = format!(
                                                                            "{}/@for:1550({})", node_scope, index
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
                                                                                key: format!("{}/@container:527", for_scope),
                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                height: None,
                                                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                    top: (8.0) as f32,
                                                                                    right: (8.0) as f32,
                                                                                    bottom: (8.0) as f32,
                                                                                    left: (8.0) as f32,
                                                                                }),
                                                                                align_x: None,
                                                                                align_y: None,
                                                                                background: (Some(palette.colors[3]))
                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: Some({
                                                                                        let mut color = palette.colors[4];
                                                                                        color.0[3] = 0.100000;
                                                                                        color
                                                                                    }),
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
                                                                                                                "Geist".into(),
                                                                                                            ),
                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                        }),
                                                                                                    },
                                                                                                    key: format!("{}/@text:541", for_scope),
                                                                                                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[4]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: false,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: (op.target.to_owned()).to_string(),
                                                                                                });
                                                                                            children
                                                                                                .push(
                                                                                                    self
                                                                                                        .render_status_badge_20(
                                                                                                            palette,
                                                                                                            format!("{}/StatusBadge@1571", for_scope),
                                                                                                            op.disposition.to_owned(),
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
                                                                                                key: format!("{}/@layout:536", for_scope),
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
                                                                                    children
                                                                                        .push(
                                                                                            self
                                                                                                .render_digest_row_21(
                                                                                                    palette,
                                                                                                    format!("{}/DigestRow@1600", for_scope),
                                                                                                    op.op_hash.to_owned(),
                                                                                                ),
                                                                                        );
                                                                                    children
                                                                                        .push(
                                                                                            self
                                                                                                .render_digest_row_22(
                                                                                                    palette,
                                                                                                    format!("{}/DigestRow@1608", for_scope),
                                                                                                    op.proposer.to_owned(),
                                                                                                ),
                                                                                        );
                                                                                    if (!(op.trace).is_empty()) {
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
                                                                                                        key: format!("{}/@text:604", for_scope),
                                                                                                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                        color: Some(palette.colors[5]),
                                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                                            monospace: false,
                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                        },
                                                                                                        width: None,
                                                                                                        align_x: None,
                                                                                                        content: ("dispatch".to_owned()).to_string(),
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
                                                                                                                    "Geist Mono".into(),
                                                                                                                ),
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                            }),
                                                                                                        },
                                                                                                        key: format!("{}/@text:610", for_scope),
                                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                        color: Some(palette.colors[5]),
                                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                                            monospace: false,
                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                        },
                                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                        align_x: None,
                                                                                                        content: (op.trace.to_owned()).to_string(),
                                                                                                    });
                                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                                    max_width: None,
                                                                                                    clip: false,
                                                                                                    key: format!("{}/@layout:599", for_scope),
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
                                                                                            });
                                                                                    }
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
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                                }),
                                                                                            },
                                                                                            key: format!("{}/@text:620", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[4]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: false,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                            align_x: None,
                                                                                            content: (op.payload.to_owned()).to_string(),
                                                                                        });
                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                        max_width: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@layout:535", for_scope),
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
                                                                            });
                                                                    }
                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:493", node_scope),
                                                                        wrap: None,
                                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                        spacing: Some((6.0) as f32),
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
                                                    ::ducktape_view_guest::wire::Node::Stack {
                                                        key: format!("{}/@layout:481", node_scope),
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        padding: None,
                                                        background: None,
                                                        border: None,
                                                        clip: false,
                                                        under: 0u32,
                                                        children: children,
                                                    }
                                                }),
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:438", node_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                            spacing: Some((0.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: Some(::ducktape_view_guest::wire::Length::Fill),
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
                                key: format!("{}/@layout:370", node_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((11.0) as f32),
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (18.0) as f32,
                                    right: (24.0) as f32,
                                    bottom: (18.0) as f32,
                                    left: (24.0) as f32,
                                }),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:163", node_scope),
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
impl ExplorerView {
    pub(crate) fn render_icon_1(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                ();
                {
                    children
                        .push({
                            let (hash, bytes) = ::ducktape_view_guest::slots::picture(
                                crate::host::icon(
                                    ::std::convert::AsRef::as_ref(&("search")),
                                ),
                            );
                            ::ducktape_view_guest::wire::Node::Svg {
                                inherit_button_ink: false,
                                key: format!("{}/@media:22", use_scope),
                                hash: hash,
                                bytes: bytes,
                                label: None,
                                color: Some(palette.colors[73]),
                                hover: None,
                                fit: None,
                                rotation: None,
                                opacity: None,
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((16.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((16.0) as f32),
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
impl ExplorerView {
    pub(crate) fn render_screen_title_0(
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
                        key: format!("{}/@text:45", use_scope),
                        size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[7]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                        },
                        width: None,
                        align_x: None,
                        content: ("Explorer".to_owned()).to_string(),
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
                            max_width: Some(((620.0) as f32).max(0.0).min(f32::MAX)),
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:52", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
                            padding: None,
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
                                key: format!("{}/@text:53", use_scope),
                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[70]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("Search everything this workspace has recorded, or read the blocks that carried operations — an idle block keeps no row, so heights skip — newest first, each one openable for the ops it carried."
                                    .to_owned())
                                    .to_string(),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
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
            }
        }
    }
    pub(crate) fn render_filter_chip_2(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.kind == "all") {
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
                            key: format!("{}/@container:86", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (6.0) as f32,
                                right: (11.0) as f32,
                                bottom: (6.0) as f32,
                                left: (11.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[7]),
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
                                                    "Geist".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:95", use_scope),
                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[9]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("All".to_owned()).to_string(),
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
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:101", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (((self.hits).len() as i64)).to_string(),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:94", use_scope),
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
                if (!(self.kind == "all")) {
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
                            key: format!("{}/@container:108", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (6.0) as f32,
                                right: (11.0) as f32,
                                bottom: (6.0) as f32,
                                left: (11.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[3]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[39]),
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
                                                    "Geist".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:117", use_scope),
                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[13]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("All".to_owned()).to_string(),
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
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:123", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[73]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (((self.hits).len() as i64)).to_string(),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:116", use_scope),
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
    pub(crate) fn render_filter_chip_3(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
        arg_1: i64,
        arg_2: bool,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if arg_2 {
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
                            key: format!("{}/@container:86", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (6.0) as f32,
                                right: (11.0) as f32,
                                bottom: (6.0) as f32,
                                left: (11.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[7]),
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
                                                    "Geist".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:95", use_scope),
                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[9]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (arg_0.to_owned()).to_string(),
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
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:101", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (arg_1).to_string(),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:94", use_scope),
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
                if (!arg_2) {
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
                            key: format!("{}/@container:108", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (6.0) as f32,
                                right: (11.0) as f32,
                                bottom: (6.0) as f32,
                                left: (11.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[3]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[39]),
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
                                                    "Geist".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:117", use_scope),
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
                                        key: format!("{}/@text:123", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[73]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (arg_1).to_string(),
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:116", use_scope),
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
    pub(crate) fn render_explorer_kind_plate_4(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
        arg_1: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (arg_0 == "page") {
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
                            key: format!("{}/@container:251", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[119]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:259", use_scope),
                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[118]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_1.to_owned()).to_string(),
                            }),
                        });
                }
                if ((!(arg_0 == "page")) && (arg_0 == "code")) {
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
                            key: format!("{}/@container:266", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[121]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:274", use_scope),
                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[120]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_1.to_owned()).to_string(),
                            }),
                        });
                }
                if ((!((arg_0 == "page") || (arg_0 == "code"))) && (arg_0 == "file")) {
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
                            key: format!("{}/@container:281", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[123]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:289", use_scope),
                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[122]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_1.to_owned()).to_string(),
                            }),
                        });
                }
                if ((!(((arg_0 == "page") || (arg_0 == "code")) || (arg_0 == "file")))
                    && (arg_0 == "run"))
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
                            key: format!("{}/@container:296", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[125]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:304", use_scope),
                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[124]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_1.to_owned()).to_string(),
                            }),
                        });
                }
                if ((!((((arg_0 == "page") || (arg_0 == "code")) || (arg_0 == "file"))
                    || (arg_0 == "run"))) && (arg_0 == "task"))
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
                            key: format!("{}/@container:311", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[127]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:319", use_scope),
                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[126]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_1.to_owned()).to_string(),
                            }),
                        });
                }
                if (!(((((arg_0 == "page") || (arg_0 == "code")) || (arg_0 == "file"))
                    || (arg_0 == "run")) || (arg_0 == "task")))
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
                            key: format!("{}/@container:326", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[77]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                    ((8.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:334", use_scope),
                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[76]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_1.to_owned()).to_string(),
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
    pub(crate) fn render_explorer_kind_badge_5(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (arg_0 == "page") {
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
                            key: format!("{}/@container:345", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (2.0) as f32,
                                right: (5.0) as f32,
                                bottom: (2.0) as f32,
                                left: (5.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[119]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:351", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[118]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("PAGE".to_owned()).to_string(),
                            }),
                        });
                }
                if ((!(arg_0 == "page")) && (arg_0 == "code")) {
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
                            key: format!("{}/@container:358", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (2.0) as f32,
                                right: (5.0) as f32,
                                bottom: (2.0) as f32,
                                left: (5.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[121]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:364", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[120]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("CODE".to_owned()).to_string(),
                            }),
                        });
                }
                if ((!((arg_0 == "page") || (arg_0 == "code"))) && (arg_0 == "file")) {
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
                            key: format!("{}/@container:371", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (2.0) as f32,
                                right: (5.0) as f32,
                                bottom: (2.0) as f32,
                                left: (5.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[123]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:377", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[122]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("FILE".to_owned()).to_string(),
                            }),
                        });
                }
                if ((!(((arg_0 == "page") || (arg_0 == "code")) || (arg_0 == "file")))
                    && (arg_0 == "run"))
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
                            key: format!("{}/@container:384", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (2.0) as f32,
                                right: (5.0) as f32,
                                bottom: (2.0) as f32,
                                left: (5.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[125]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:390", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[124]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("RUN".to_owned()).to_string(),
                            }),
                        });
                }
                if ((!((((arg_0 == "page") || (arg_0 == "code")) || (arg_0 == "file"))
                    || (arg_0 == "run"))) && (arg_0 == "task"))
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
                            key: format!("{}/@container:397", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (2.0) as f32,
                                right: (5.0) as f32,
                                bottom: (2.0) as f32,
                                left: (5.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[127]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:403", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[126]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("TASK".to_owned()).to_string(),
                            }),
                        });
                }
                if (!(((((arg_0 == "page") || (arg_0 == "code")) || (arg_0 == "file"))
                    || (arg_0 == "run")) || (arg_0 == "task")))
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
                            key: format!("{}/@container:410", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (2.0) as f32,
                                right: (5.0) as f32,
                                bottom: (2.0) as f32,
                                left: (5.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[77]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
                                    ((4.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:416", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[76]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("MESSAGE".to_owned()).to_string(),
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
    pub(crate) fn render_explorer_card_6(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::ExplorerHit,
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
                        .push(
                            self
                                .render_explorer_kind_plate_4(
                                    palette,
                                    format!("{}/ExplorerKindPlate@766", use_scope),
                                    arg_0.kind.to_owned(),
                                    arg_0.code.to_owned(),
                                ),
                        );
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
                                            key: format!("{}/@text:153", use_scope),
                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[7]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            align_x: None,
                                            content: (arg_0.title.to_owned()).to_string(),
                                        });
                                    children
                                        .push(
                                            self
                                                .render_explorer_kind_badge_5(
                                                    palette,
                                                    format!("{}/ExplorerKindBadge@780", use_scope),
                                                    arg_0.kind.to_owned(),
                                                ),
                                        );
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:148", use_scope),
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
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::Word),
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
                                    key: format!("{}/@text:164", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[41]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align_x: None,
                                    content: (arg_0.snippet.to_owned()).to_string(),
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
                                    key: format!("{}/@text:171", use_scope),
                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[73]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align_x: None,
                                    content: (arg_0.meta.to_owned()).to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:147", use_scope),
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
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:141", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((12.0) as f32),
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
                    key: format!("{}/@text:14", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("Nothing of that kind matched — the other chips still hold results."
                        .to_owned())
                        .to_string(),
                }),
            }
        }
    }
    pub(crate) fn render_empty_state_8(
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
                    key: format!("{}/@text:14", use_scope),
                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[71]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: ("Nothing matched that query in this workspace.".to_owned())
                        .to_string(),
                }),
            }
        }
    }
    pub(crate) fn render_empty_state_9(
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
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (22.0) as f32,
                    right: (22.0) as f32,
                    bottom: (22.0) as f32,
                    left: (22.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
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
                            key: format!("{}/@container:29", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((42.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((42.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[3]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[39]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:39", use_scope),
                                size: Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[7]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("◇".to_owned()).to_string(),
                            }),
                        });
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
                            key: format!("{}/@text:40", use_scope),
                            size: Some(16.0f32),
                            color: Some(palette.colors[4]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            },
                            width: None,
                            align_x: None,
                            content: ("Not connected".to_owned()).to_string(),
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
                            key: format!("{}/@text:41", use_scope),
                            size: Some(12.5f32),
                            color: Some(palette.colors[5]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Click the network name in the titlebar to pick or reconnect a network."
                                .to_owned())
                                .to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:24", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((7.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
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
    pub(crate) fn render_empty_state_10(
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
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (22.0) as f32,
                    right: (22.0) as f32,
                    bottom: (22.0) as f32,
                    left: (22.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
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
                            key: format!("{}/@container:29", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((42.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((42.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[3]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[39]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:39", use_scope),
                                size: Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[7]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("◇".to_owned()).to_string(),
                            }),
                        });
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
                            key: format!("{}/@text:40", use_scope),
                            size: Some(16.0f32),
                            color: Some(palette.colors[4]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            },
                            width: None,
                            align_x: None,
                            content: ("No blocks yet".to_owned()).to_string(),
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
                            key: format!("{}/@text:41", use_scope),
                            size: Some(12.5f32),
                            color: Some(palette.colors[5]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Blocks that carried operations appear here as they finalize."
                                .to_owned())
                                .to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:24", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((7.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
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
    pub(crate) fn render_empty_state_13(
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
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (22.0) as f32,
                    right: (22.0) as f32,
                    bottom: (22.0) as f32,
                    left: (22.0) as f32,
                }),
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                background: (None).map(::ducktape_view_guest::wire::Background::Color),
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
                            key: format!("{}/@container:29", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((42.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((42.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[3]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[39]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
                                    ((21.0) as f32).max(0.0).min(f32::MAX),
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
                                key: format!("{}/@text:39", use_scope),
                                size: Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[7]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: ("◇".to_owned()).to_string(),
                            }),
                        });
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
                            key: format!("{}/@text:40", use_scope),
                            size: Some(16.0f32),
                            color: Some(palette.colors[4]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            },
                            width: None,
                            align_x: None,
                            content: ("Select a block".to_owned()).to_string(),
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
                            key: format!("{}/@text:41", use_scope),
                            size: Some(12.5f32),
                            color: Some(palette.colors[5]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: None,
                            align_x: None,
                            content: ("Its operations and dispatch traces appear here."
                                .to_owned())
                                .to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:24", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((7.0) as f32),
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
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
    pub(crate) fn render_badge_success_16(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
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
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[27]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[28]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
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
                            key: format!("{}/@container:189", use_scope),
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
                            key: format!("{}/@text:196", use_scope),
                            size: Some(9.0f32),
                            color: Some(palette.colors[4]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:188", use_scope),
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
    pub(crate) fn render_badge_warning_17(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
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
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
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
                            key: format!("{}/@container:208", use_scope),
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
                            key: format!("{}/@text:215", use_scope),
                            size: Some(9.0f32),
                            color: Some(palette.colors[4]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:207", use_scope),
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
    pub(crate) fn render_badge_destructive_18(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
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
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[22]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[23]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
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
                            key: format!("{}/@container:227", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[24]))
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
                            key: format!("{}/@text:234", use_scope),
                            size: Some(9.0f32),
                            color: Some(palette.colors[4]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:226", use_scope),
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
    pub(crate) fn render_badge_outline_19(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
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
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
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
                    key: format!("{}/@text:245", use_scope),
                    size: Some(9.0f32),
                    color: Some(palette.colors[13]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                    },
                    width: None,
                    align_x: None,
                    content: (arg_0.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(crate) fn render_status_badge_20(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            if (arg_0 == "active") {
                children
                    .push(
                        self
                            .render_badge_success_16(
                                palette,
                                format!("{}/Badge.Success@683", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            if ((!(arg_0 == "active")) && (arg_0 == "paused")) {
                children
                    .push(
                        self
                            .render_badge_warning_17(
                                palette,
                                format!("{}/Badge.Warning@685", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            if ((!((arg_0 == "active") || (arg_0 == "paused"))) && (arg_0 == "open")) {
                children
                    .push(
                        self
                            .render_badge_success_16(
                                palette,
                                format!("{}/Badge.Success@687", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            if ((!(((arg_0 == "active") || (arg_0 == "paused")) || (arg_0 == "open")))
                && (arg_0 == "closed"))
            {
                children
                    .push(
                        self
                            .render_badge_destructive_18(
                                palette,
                                format!("{}/Badge.Destructive@689", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            if ((!((((arg_0 == "active") || (arg_0 == "paused")) || (arg_0 == "open"))
                || (arg_0 == "closed"))) && (arg_0 == "merged"))
            {
                children
                    .push(
                        self
                            .render_badge_success_16(
                                palette,
                                format!("{}/Badge.Success@691", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            if ((!(((((arg_0 == "active") || (arg_0 == "paused")) || (arg_0 == "open"))
                || (arg_0 == "closed")) || (arg_0 == "merged"))) && (arg_0 == "passed"))
            {
                children
                    .push(
                        self
                            .render_badge_success_16(
                                palette,
                                format!("{}/Badge.Success@693", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            if ((!((((((arg_0 == "active") || (arg_0 == "paused")) || (arg_0 == "open"))
                || (arg_0 == "closed")) || (arg_0 == "merged")) || (arg_0 == "passed")))
                && (arg_0 == "rejected"))
            {
                children
                    .push(
                        self
                            .render_badge_destructive_18(
                                palette,
                                format!("{}/Badge.Destructive@695", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            if ((!(((((((arg_0 == "active") || (arg_0 == "paused")) || (arg_0 == "open"))
                || (arg_0 == "closed")) || (arg_0 == "merged")) || (arg_0 == "passed"))
                || (arg_0 == "rejected"))) && (arg_0 == "applied"))
            {
                children
                    .push(
                        self
                            .render_badge_success_16(
                                palette,
                                format!("{}/Badge.Success@697", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            if ((!((((((((arg_0 == "active") || (arg_0 == "paused"))
                || (arg_0 == "open")) || (arg_0 == "closed")) || (arg_0 == "merged"))
                || (arg_0 == "passed")) || (arg_0 == "rejected"))
                || (arg_0 == "applied"))) && (arg_0 == "discarded"))
            {
                children
                    .push(
                        self
                            .render_badge_warning_17(
                                palette,
                                format!("{}/Badge.Warning@699", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            if (!(((((((((arg_0 == "active") || (arg_0 == "paused"))
                || (arg_0 == "open")) || (arg_0 == "closed")) || (arg_0 == "merged"))
                || (arg_0 == "passed")) || (arg_0 == "rejected"))
                || (arg_0 == "applied")) || (arg_0 == "discarded")))
            {
                children
                    .push(
                        self
                            .render_badge_outline_19(
                                palette,
                                format!("{}/Badge.Outline@701", use_scope),
                                arg_0.to_owned(),
                            ),
                    );
            }
            ::ducktape_view_guest::wire::Node::Linear {
                max_width: None,
                clip: false,
                key: format!("{}/@layout:60", use_scope),
                wrap: None,
                axis: ::ducktape_view_guest::wire::Axis::Row,
                spacing: None,
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
ducktape_view_guest::export_app!(
    ExplorerView, "Explorer",
    "The ledger this network wrote: blocks, their operations, and a search over the workspace.",
    ["explorer"]
);
