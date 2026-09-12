//! The Approvals screen as a module-owned view: every decision the network
//! is being asked to make, and the ones it has settled, rendered from a
//! wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`governance.props`: connected,
//! admin, dark). The view reads its own register through the kernel's
//! `rpc.query` / `rpc.blocks`, re-reads it on every `rpc.live` hit for the
//! governance plane, and a vote or a settle leaves as `op.submit` — the
//! governance message the kernel signs with the seated key. The endpoint,
//! the key and the password never cross: a guest that sees no key cannot
//! leak one.
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
pub struct GovernanceView {
    pub(crate) active_palette: AppTheme,
    pub(crate) rows: Vec<crate::host::ProposalRow>,
    pub(crate) voting: String,
    pub(crate) admin: bool,
    pub(crate) connected: bool,
    pub(crate) connection_serial: i64,
    pub(crate) answered: bool,
    pub(crate) host_error: String,
    pub(crate) badge_sent: bool,
}
impl ::std::fmt::Debug for GovernanceView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("GovernanceView")
    }
}
#[derive(Clone)]
pub enum Message {
    SessionArrived(crate::host::SessionItem),
    RegisterArrived(crate::host::RegisterItem),
    ActDone(crate::host::ActItem),
    GovVote(String, bool),
    GovExecute(String),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl GovernanceView {
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
impl GovernanceView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            rows: Vec::new(),
            voting: "".to_owned(),
            admin: false,
            connected: false,
            connection_serial: 0,
            answered: false,
            host_error: "".to_owned(),
            badge_sent: false,
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    #[allow(clippy::too_many_arguments)]
    fn restore_state(
        active_palette: AppTheme,
        rows: Vec<crate::host::ProposalRow>,
        voting: String,
        admin: bool,
        connected: bool,
        connection_serial: i64,
        answered: bool,
        host_error: String,
        badge_sent: bool,
    ) -> Self {
        Self {
            active_palette: active_palette,
            rows: rows,
            voting: voting,
            admin: admin,
            connected: connected,
            connection_serial: connection_serial,
            answered: answered,
            host_error: host_error,
            badge_sent: badge_sent,
        }
    }
    pub(crate) const SNAPSHOT_SCHEMA: &'static str = "7c12db27b05b027805b40f4d493f95bcbf83f7b71fb9a350d90ef241043cbc72";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("GovernanceView"),
                fields: vec![
                    (String::from("active_palette"), match & self.active_palette {
                    AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("AppTheme"), fields : vec![(String::from("app"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    AppTheme::AppDark =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AppTheme"), fields : vec![(String::from("app_dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("rows"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.rows).iter()
                    .map(| item | ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("ProposalRow"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("action"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).action))), (String::from("detail"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).detail))), (String::from("proposer"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).proposer))), (String::from("status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).status))), (String::from("deadline"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .deadline))), (String::from("approvals"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .approvals))), (String::from("rejections"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .rejections))), (String::from("rule"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).rule))), (String::from("required_yes"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .required_yes))), (String::from("electorate"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .electorate))), (String::from("open"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).open))),
                    (String::from("settled_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .settled_height)))] }).collect())), (String::from("voting"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.voting))), (String::from("admin"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.admin))),
                    (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected))), (String::from("connection_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .connection_serial))), (String::from("answered"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .answered))), (String::from("host_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.host_error))), (String::from("badge_sent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .badge_sent)))
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
            if name != "GovernanceView" || fields.len() != 9 {
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
            if name != "rows" {
                return None;
            }
            let rows: Vec<crate::host::ProposalRow> = (match value {
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
                            if name != "ProposalRow" || fields.len() != 13 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "action" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "detail" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "proposer" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "status" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "deadline" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "approvals" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "rejections" {
                                return None;
                            }
                            let (name, field_8) = fields.next()?;
                            if name != "rule" {
                                return None;
                            }
                            let (name, field_9) = fields.next()?;
                            if name != "required_yes" {
                                return None;
                            }
                            let (name, field_10) = fields.next()?;
                            if name != "electorate" {
                                return None;
                            }
                            let (name, field_11) = fields.next()?;
                            if name != "open" {
                                return None;
                            }
                            let (name, field_12) = fields.next()?;
                            if name != "settled_height" {
                                return None;
                            }
                            Some(crate::host::ProposalRow {
                                id: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                action: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                detail: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                proposer: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                status: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                deadline: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                approvals: (match field_6 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                rejections: (match field_7 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                rule: (match field_8 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                required_yes: (match field_9 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                electorate: (match field_10 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                open: (match field_11 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                settled_height: (match field_12 {
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
            if name != "voting" {
                return None;
            }
            let voting: String = (match value {
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
            if name != "answered" {
                return None;
            }
            let answered: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
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
            if name != "badge_sent" {
                return None;
            }
            let badge_sent: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            Some(
                Self::restore_state(
                    active_palette,
                    rows,
                    voting,
                    admin,
                    connected,
                    connection_serial,
                    answered,
                    host_error,
                    badge_sent,
                ),
            )
        })())
            .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl GovernanceView {
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::register(self.connection_serial)
                        .map(move |value| Message::RegisterArrived(value)),
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
                let (app, _) = GovernanceView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
impl GovernanceView {
    pub(crate) fn render_proposal_kind_pill_0(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (crate::host::proposal_kind_tone(
                    ::std::convert::AsRef::as_ref(&(arg_0)),
                ) == "access")
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
                            key: format!("{}/@container:108", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (2.0) as f32,
                                right: (6.0) as f32,
                                bottom: (2.0) as f32,
                                left: (6.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[18]))
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
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: None,
                                },
                                key: format!("{}/@text:114", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[16]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: true,
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_0.to_owned()).to_string(),
                            }),
                        });
                }
                if (crate::host::proposal_kind_tone(
                    ::std::convert::AsRef::as_ref(&(arg_0)),
                ) != "access")
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
                            key: format!("{}/@container:121", use_scope),
                            width: None,
                            height: None,
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (2.0) as f32,
                                right: (6.0) as f32,
                                bottom: (2.0) as f32,
                                left: (6.0) as f32,
                            }),
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
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
                                    wrapping: None,
                                    tracking: 0.0f32,
                                    font: None,
                                },
                                key: format!("{}/@text:127", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[100]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: true,
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                },
                                width: None,
                                align_x: None,
                                content: (arg_0.to_owned()).to_string(),
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
    pub(crate) fn render_quorum_dot_1(
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
                            key: format!("{}/@container:137", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((13.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((13.0) as f32),
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
                                    ((6.5) as f32).max(0.0).min(f32::MAX),
                                    ((6.5) as f32).max(0.0).min(f32::MAX),
                                    ((6.5) as f32).max(0.0).min(f32::MAX),
                                    ((6.5) as f32).max(0.0).min(f32::MAX),
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
                            key: format!("{}/@container:145", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((13.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((13.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[3]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[98]),
                                width: Some(((1.5) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((6.5) as f32).max(0.0).min(f32::MAX),
                                    ((6.5) as f32).max(0.0).min(f32::MAX),
                                    ((6.5) as f32).max(0.0).min(f32::MAX),
                                    ((6.5) as f32).max(0.0).min(f32::MAX),
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
    pub(crate) fn render_settled_proposal_row_2(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::ProposalRow,
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
                    color: Some(palette.colors[60]),
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
                            key: format!("{}/@container:170", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((19.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((19.0) as f32),
                            ),
                            padding: None,
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                            background: (Some(palette.colors[27]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: Some(palette.colors[28]),
                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                radius: Some([
                                    ((9.5) as f32).max(0.0).min(f32::MAX),
                                    ((9.5) as f32).max(0.0).min(f32::MAX),
                                    ((9.5) as f32).max(0.0).min(f32::MAX),
                                    ((9.5) as f32).max(0.0).min(f32::MAX),
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
                                    font: None,
                                },
                                key: format!("{}/@text:180", use_scope),
                                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[25]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: true,
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
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
                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:186", use_scope),
                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[5]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                            },
                            width: None,
                            align_x: None,
                            content: (arg_0.id.to_owned()).to_string(),
                        });
                    children
                        .push(::ducktape_view_guest::wire::Node::Space {
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: None,
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
                                        wrapping: None,
                                        tracking: 0.0f32,
                                        font: None,
                                    },
                                    key: format!("{}/@text:193", use_scope),
                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: true,
                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (arg_0.status.to_owned()).to_string(),
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
                                        font: None,
                                    },
                                    key: format!("{}/@text:199", use_scope),
                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: true,
                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("·".to_owned()).to_string(),
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
                                        font: None,
                                    },
                                    key: format!("{}/@text:205", use_scope),
                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: true,
                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (crate::host::tally_label(
                                        arg_0.approvals,
                                        arg_0.required_yes,
                                    ))
                                        .to_string(),
                                });
                            if (arg_0.settled_height > 0) {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: None,
                                            tracking: 0.0f32,
                                            font: None,
                                        },
                                        key: format!("{}/@text:215", use_scope),
                                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: true,
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ("·".to_owned()).to_string(),
                                    });
                            }
                            if (arg_0.settled_height > 0) {
                                children
                                    .push(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: None,
                                            tracking: 0.0f32,
                                            font: None,
                                        },
                                        key: format!("{}/@text:222", use_scope),
                                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: true,
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (crate::host::height_label_short(
                                            arg_0.settled_height,
                                        ))
                                            .to_string(),
                                    });
                            }
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:192", use_scope),
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
                        key: format!("{}/@layout:165", use_scope),
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
                }),
            }
        }
    }
}
impl GovernanceView {
    #[allow(clippy::assign_op_pattern)]
    pub(crate) fn update(
        &mut self,
        message: Message,
    ) -> ::ducktape_view_guest::Task<Message> {
        match message {
            Message::SessionArrived(item) => self.on_session_arrived(item),
            Message::RegisterArrived(item) => self.on_register_arrived(item),
            Message::ActDone(item) => self.on_act_done(item),
            Message::GovVote(proposal_id, approve) => {
                self.on_gov_vote(proposal_id, approve)
            }
            Message::GovExecute(proposal_id) => self.on_gov_execute(proposal_id),
        }
    }
    fn on_session_arrived(
        &mut self,
        item: crate::host::SessionItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.host_error = item.error.to_owned();
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            let next = item.next.clone();
            self.connection_serial = crate::host::connection_serial_after(
                self.connected,
                next.connected,
                self.connection_serial,
            );
            self.admin = next.admin;
            self.connected = next.connected;
            self.active_palette = AppTheme::App;
            if (!next.dark) {
                return ::ducktape_view_guest::Task::none();
            }
            self.active_palette = AppTheme::AppDark;
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_register_arrived(
        &mut self,
        item: crate::host::RegisterItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.host_error = item.error.to_owned();
            self.answered = true;
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            self.rows = item.rows.clone();
            self.badge_sent = (crate::host::badge(
                crate::host::open_proposals(::std::convert::AsRef::as_ref(&(self.rows))),
            ));
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_act_done(
        &mut self,
        item: crate::host::ActItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.voting = "".to_owned();
            self.host_error = item.error.to_owned();
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_gov_vote(
        &mut self,
        proposal_id: String,
        approve: bool,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((!self.connected) || (!(self.voting).is_empty())) {
                return ::ducktape_view_guest::Task::none();
            }
            self.voting = proposal_id.to_owned();
            let _sent = (crate::host::vote(proposal_id.to_owned(), approve));
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_gov_execute(
        &mut self,
        proposal_id: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((!self.connected) || (!(self.voting).is_empty())) {
                return ::ducktape_view_guest::Task::none();
            }
            self.voting = proposal_id.to_owned();
            let _sent = (crate::host::execute(proposal_id.to_owned()));
            ::ducktape_view_guest::Task::none()
        }
    }
}
impl GovernanceView {
    pub(crate) fn view(&self) -> ::ducktape_view_guest::wire::Node {
        let palette = self.palette();
        {
            let node_scope = format!("{}/root", "GovernanceView");
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
                            key: format!("{}/@container:239", node_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((56.0) as f32),
                            ),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (0.0) as f32,
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
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                children
                                    .push({
                                        let node_scope = format!("{}/seal", node_scope);
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
                                                ::ducktape_view_guest::wire::Length::Fixed((24.0) as f32),
                                            ),
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((24.0) as f32),
                                            ),
                                            padding: None,
                                            align_x: None,
                                            align_y: None,
                                            background: (None)
                                                .map(::ducktape_view_guest::wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new({
                                                let node_scope = format!("{}/seal-svg", node_scope);
                                                ::ducktape_view_guest::wire::Node::Surface {
                                                    key: node_scope.clone(),
                                                    name: String::from("artifact_svg"),
                                                    args: ::std::vec![
                                                        { let surface_arg = & ("icons/seal.svg".to_owned());
                                                        ::ducktape_view_guest::wire::SurfaceValue::Str(::std::string::ToString::to_string(surface_arg))
                                                        }
                                                    ],
                                                    on_event: None,
                                                }
                                            }),
                                        }
                                    });
                                children
                                    .push({
                                        let node_scope = format!("{}/title", node_scope);
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
                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                }),
                                            },
                                            key: node_scope.clone(),
                                            size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[7]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: ("Approvals".to_owned()).to_string(),
                                        }
                                    });
                                children
                                    .push({
                                        let node_scope = format!("{}/meta", node_scope);
                                        ::ducktape_view_guest::wire::Node::Text {
                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                height: None,
                                                align_y: None,
                                                line_height: None,
                                                shaping: None,
                                                wrapping: None,
                                                tracking: 0.0f32,
                                                font: None,
                                            },
                                            key: node_scope.clone(),
                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[72]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: true,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: (crate::host::proposals_summary(
                                                self.connected,
                                                ::std::convert::AsRef::as_ref(&(self.rows)),
                                            ))
                                                .to_string(),
                                        }
                                    });
                                if (self.connected
                                    && (crate::host::open_proposals(
                                        ::std::convert::AsRef::as_ref(&(self.rows)),
                                    ) > 0))
                                {
                                    children
                                        .push({
                                            let node_scope = format!("{}/pending", node_scope);
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
                                                background: (Some(palette.colors[18]))
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: Some(::ducktape_view_guest::wire::Border {
                                                    color: None,
                                                    width: None,
                                                    radius: Some([
                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
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
                                                        font: None,
                                                    },
                                                    key: format!("{}/@text:269", node_scope),
                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: Some(palette.colors[16]),
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: true,
                                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: (crate::host::pending_label(
                                                        ::std::convert::AsRef::as_ref(&(self.rows)),
                                                    ))
                                                        .to_string(),
                                                }),
                                            }
                                        });
                                }
                                children
                                    .push(::ducktape_view_guest::wire::Node::Space {
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                    });
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:244", node_scope),
                                    wrap: None,
                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                    spacing: Some((10.0) as f32),
                                    padding: None,
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                    background: None,
                                    border: None,
                                    children: children,
                                }
                            }),
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
                            key: format!("{}/@container:276", node_scope),
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
                            let node_scope = format!("{}/approvals-body", node_scope);
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
                                    if (self.connected && (!self.admin)) {
                                        children
                                            .push({
                                                let node_scope = format!("{}/gate", node_scope);
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
                                                                        key: format!("{}/@container:311", node_scope),
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
                                                                    key: format!("{}/@layout:310", node_scope),
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
                                                                        key: format!("{}/@text:319", node_scope),
                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[31]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        align_x: None,
                                                                        content: ("Approval votes are cast by this network's validators, and this node does not hold validator standing."
                                                                            .to_owned())
                                                                            .to_string(),
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
                                                                        key: format!("{}/@text:324", node_scope),
                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[30]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        align_x: None,
                                                                        content: ("You can still read every proposal and follow its tally while it runs."
                                                                            .to_owned())
                                                                            .to_string(),
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:318", node_scope),
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
                                                            key: format!("{}/@layout:305", node_scope),
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
                                            });
                                    }
                                    if (!self.connected) {
                                        children
                                            .push({
                                                let node_scope = format!("{}/offline", node_scope);
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
                                                                key: format!("{}/@container:343", node_scope),
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
                                                                    key: format!("{}/@text:353", node_scope),
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
                                                                    line_height: None,
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
                                                                key: format!("{}/@text:354", node_scope),
                                                                size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
                                                                key: format!("{}/@text:359", node_scope),
                                                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[70]),
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
                                                            key: format!("{}/@layout:338", node_scope),
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
                                            });
                                    }
                                    if ((self.connected && (self.rows).is_empty())
                                        && self.answered)
                                    {
                                        children
                                            .push({
                                                let node_scope = format!("{}/empty", node_scope);
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
                                                    background: (None)
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
                                                        key: format!("{}/@text:374", node_scope),
                                                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[71]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("No proposals yet — a membership or configuration change opens the first one."
                                                            .to_owned())
                                                            .to_string(),
                                                    }),
                                                }
                                            });
                                    }
                                    if (((self.connected
                                        && (crate::host::open_proposals(
                                            ::std::convert::AsRef::as_ref(&(self.rows)),
                                        ) <= 0)) && (!(self.rows).is_empty())) && self.answered)
                                    {
                                        children
                                            .push({
                                                let node_scope = format!("{}/all-settled", node_scope);
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
                                                    background: (None)
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
                                                        key: format!("{}/@text:387", node_scope),
                                                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[71]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("No proposals waiting — every decision on this network is finalized."
                                                            .to_owned())
                                                            .to_string(),
                                                    }),
                                                }
                                            });
                                    }
                                    if (self.connected
                                        && (crate::host::open_proposals(
                                            ::std::convert::AsRef::as_ref(&(self.rows)),
                                        ) > 0))
                                    {
                                        children
                                            .push({
                                                let node_scope = format!("{}/open", node_scope);
                                                {
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    for (index, proposal) in self.rows.iter().enumerate() {
                                                        let for_scope = format!(
                                                            "{}/@for:862({})", node_scope, index
                                                        );
                                                        if proposal.open {
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
                                                                    key: format!("{}/@container:395", for_scope),
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                                        top: (16.0) as f32,
                                                                        right: (16.0) as f32,
                                                                        bottom: (16.0) as f32,
                                                                        left: (16.0) as f32,
                                                                    }),
                                                                    align_x: None,
                                                                    align_y: None,
                                                                    background: (Some(palette.colors[3]))
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
                                                                    content: Box::new({
                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                        children
                                                                            .push({
                                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                children
                                                                                    .push(
                                                                                        self
                                                                                            .render_proposal_kind_pill_0(
                                                                                                palette,
                                                                                                format!("{}/ProposalKindPill@880", for_scope),
                                                                                                proposal.action.to_owned(),
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
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: format!("{}/@text:412", for_scope),
                                                                                        size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[7]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                        },
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        align_x: None,
                                                                                        content: (proposal.id.to_owned()).to_string(),
                                                                                    });
                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                    max_width: None,
                                                                                    clip: false,
                                                                                    key: format!("{}/@layout:404", for_scope),
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
                                                                                        key: format!("{}/@text:425", for_scope),
                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[70]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: ("proposed by".to_owned()).to_string(),
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
                                                                                                key: format!("{}/@text:427", for_scope),
                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[13]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("@".to_owned()).to_string(),
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
                                                                                                key: format!("{}/@text:428", for_scope),
                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[13]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: (proposal.proposer.to_owned()).to_string(),
                                                                                            });
                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                            max_width: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@layout:426", for_scope),
                                                                                            wrap: None,
                                                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                            spacing: Some((0.0) as f32),
                                                                                            padding: None,
                                                                                            width: None,
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
                                                                                        key: format!("{}/@text:429", for_scope),
                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[70]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: ("· expires at h".to_owned()).to_string(),
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
                                                                                            font: None,
                                                                                        },
                                                                                        key: format!("{}/@text:430", for_scope),
                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[13]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: true,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: (proposal.deadline).to_string(),
                                                                                    });
                                                                                if (!(proposal.detail).is_empty()) {
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
                                                                                            key: format!("{}/@text:437", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[70]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: false,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: ("·".to_owned()).to_string(),
                                                                                        });
                                                                                }
                                                                                if (!(proposal.detail).is_empty()) {
                                                                                    children
                                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                height: None,
                                                                                                align_y: None,
                                                                                                line_height: None,
                                                                                                shaping: None,
                                                                                                wrapping: None,
                                                                                                tracking: 0.0f32,
                                                                                                font: None,
                                                                                            },
                                                                                            key: format!("{}/@text:439", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[13]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: true,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                            },
                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                            align_x: None,
                                                                                            content: (proposal.detail.to_owned()).to_string(),
                                                                                        });
                                                                                }
                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                    max_width: None,
                                                                                    clip: false,
                                                                                    key: format!("{}/@layout:420", for_scope),
                                                                                    wrap: None,
                                                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                    spacing: Some((4.0) as f32),
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
                                                                                        for (index, seat) in crate::host::quorum_dots(
                                                                                                proposal.approvals,
                                                                                                proposal.required_yes,
                                                                                            )
                                                                                            .iter()
                                                                                            .enumerate()
                                                                                        {
                                                                                            let for_scope = format!(
                                                                                                "{}/@for:924({})", for_scope, index
                                                                                            );
                                                                                            children
                                                                                                .push(
                                                                                                    self
                                                                                                        .render_quorum_dot_1(
                                                                                                            palette,
                                                                                                            format!("{}/QuorumDot@925", for_scope),
                                                                                                            seat.filled,
                                                                                                        ),
                                                                                                );
                                                                                        }
                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                            max_width: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@layout:454", for_scope),
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
                                                                                if (crate::host::tally_tone(
                                                                                    proposal.approvals,
                                                                                    proposal.required_yes,
                                                                                ) == "near")
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
                                                                                                font: None,
                                                                                            },
                                                                                            key: format!("{}/@text:460", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[25]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: true,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: (crate::host::tally_label(
                                                                                                proposal.approvals,
                                                                                                proposal.required_yes,
                                                                                            ))
                                                                                                .to_string(),
                                                                                        });
                                                                                }
                                                                                if (crate::host::tally_tone(
                                                                                    proposal.approvals,
                                                                                    proposal.required_yes,
                                                                                ) != "near")
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
                                                                                                font: None,
                                                                                            },
                                                                                            key: format!("{}/@text:467", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[71]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: true,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: (crate::host::tally_label(
                                                                                                proposal.approvals,
                                                                                                proposal.required_yes,
                                                                                            ))
                                                                                                .to_string(),
                                                                                        });
                                                                                }
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
                                                                                        key: format!("{}/@text:473", for_scope),
                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[71]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: (crate::host::tally_note(
                                                                                            proposal.approvals,
                                                                                            proposal.required_yes,
                                                                                        ))
                                                                                            .to_string(),
                                                                                    });
                                                                                if (proposal.rejections > 0) {
                                                                                    children
                                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                                height: None,
                                                                                                align_y: None,
                                                                                                line_height: None,
                                                                                                shaping: None,
                                                                                                wrapping: None,
                                                                                                tracking: 0.0f32,
                                                                                                font: None,
                                                                                            },
                                                                                            key: format!("{}/@text:478", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[20]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: true,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: (proposal.rejections).to_string(),
                                                                                        });
                                                                                }
                                                                                if (proposal.rejections > 0) {
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
                                                                                            key: format!("{}/@text:485", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[70]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: false,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: ("against".to_owned()).to_string(),
                                                                                        });
                                                                                }
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Space {
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        height: None,
                                                                                    });
                                                                                children
                                                                                    .push({
                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                        children
                                                                                            .push({
                                                                                                let node_scope = format!("{}/reject", node_scope);
                                                                                                ::ducktape_view_guest::wire::Node::Button {
                                                                                                    checked: None,
                                                                                                    expanded: None,
                                                                                                    description: None,
                                                                                                    key: node_scope.clone(),
                                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                                                        Box::new(::ducktape_view_guest::wire::Node::Text {
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
                                                                                                            key: format!("{}/@text:499", for_scope),
                                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[13]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: false,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: ("Reject".to_owned()).to_string(),
                                                                                                        }),
                                                                                                    ),
                                                                                                    label: Some(String::from("Reject".to_owned())),
                                                                                                    on_press: if ((!(self.voting).is_empty())) {
                                                                                                        None
                                                                                                    } else {
                                                                                                        Some(
                                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                                    Message::GovVote(proposal.id.to_owned(), false),
                                                                                                                ),
                                                                                                            )
                                                                                                    },
                                                                                                    width: None,
                                                                                                    height: None,
                                                                                                    padding: Some(
                                                                                                        ::ducktape_view_guest::wire::Edges::all((8.0) as f32),
                                                                                                    ),
                                                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                                                background: None,
                                                                                                                text: None,
                                                                                                                border: None,
                                                                                                            },
                                                                                                            hover_background: None,
                                                                                                            pressed_background: None,
                                                                                                            disabled_background: None,
                                                                                                            disabled_text: None,
                                                                                                            disabled_opacity: None,
                                                                                                            focus_ring: None,
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
                                                                                                            background: Some(palette.colors[3]),
                                                                                                            text: Some(palette.colors[13]),
                                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                color: Some(palette.colors[40]),
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
                                                                                                            text: Some(palette.colors[13]),
                                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                color: Some(palette.colors[94]),
                                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                                radius: Some([
                                                                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                ]),
                                                                                                            }),
                                                                                                        }),
                                                                                                        pressed: None,
                                                                                                        disabled: Some(::ducktape_view_guest::wire::Face {
                                                                                                            background: Some(palette.colors[3]),
                                                                                                            text: Some(palette.colors[11]),
                                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                                color: Some(palette.colors[40]),
                                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                                radius: Some([
                                                                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                ]),
                                                                                                            }),
                                                                                                        }),
                                                                                                    },
                                                                                                }
                                                                                            });
                                                                                        if (proposal.approvals < proposal.required_yes) {
                                                                                            children
                                                                                                .push({
                                                                                                    let node_scope = format!("{}/approve", node_scope);
                                                                                                    ::ducktape_view_guest::wire::Node::Button {
                                                                                                        checked: None,
                                                                                                        expanded: None,
                                                                                                        description: None,
                                                                                                        key: node_scope.clone(),
                                                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                                                            Box::new(::ducktape_view_guest::wire::Node::Text {
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
                                                                                                                key: format!("{}/@text:509", for_scope),
                                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                color: Some(palette.colors[9]),
                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                    monospace: false,
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                },
                                                                                                                width: None,
                                                                                                                align_x: None,
                                                                                                                content: (crate::host::approve_label(
                                                                                                                    proposal.approvals,
                                                                                                                    proposal.required_yes,
                                                                                                                ))
                                                                                                                    .to_string(),
                                                                                                            }),
                                                                                                        ),
                                                                                                        label: Some(String::from("Approve".to_owned())),
                                                                                                        on_press: if ((!(self.voting).is_empty())) {
                                                                                                            None
                                                                                                        } else {
                                                                                                            Some(
                                                                                                                    ::ducktape_view_guest::slots::message(
                                                                                                                        Message::GovVote(proposal.id.to_owned(), true),
                                                                                                                    ),
                                                                                                                )
                                                                                                        },
                                                                                                        width: None,
                                                                                                        height: None,
                                                                                                        padding: Some(
                                                                                                            ::ducktape_view_guest::wire::Edges::all((8.0) as f32),
                                                                                                        ),
                                                                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                                                base: ::ducktape_view_guest::wire::Face {
                                                                                                                    background: None,
                                                                                                                    text: None,
                                                                                                                    border: None,
                                                                                                                },
                                                                                                                hover_background: None,
                                                                                                                pressed_background: None,
                                                                                                                disabled_background: None,
                                                                                                                disabled_text: None,
                                                                                                                disabled_opacity: None,
                                                                                                                focus_ring: None,
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
                                                                                                                background: Some(palette.colors[7]),
                                                                                                                text: Some(palette.colors[9]),
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
                                                                                                            },
                                                                                                            hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                                                background: Some(palette.colors[8]),
                                                                                                                text: Some(palette.colors[9]),
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
                                                                                                            }),
                                                                                                            pressed: None,
                                                                                                            disabled: Some(::ducktape_view_guest::wire::Face {
                                                                                                                background: Some(palette.colors[10]),
                                                                                                                text: Some(palette.colors[11]),
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
                                                                                                            }),
                                                                                                        },
                                                                                                    }
                                                                                                });
                                                                                        }
                                                                                        if (proposal.approvals >= proposal.required_yes) {
                                                                                            children
                                                                                                .push({
                                                                                                    let node_scope = format!("{}/settle", node_scope);
                                                                                                    ::ducktape_view_guest::wire::Node::Button {
                                                                                                        checked: None,
                                                                                                        expanded: None,
                                                                                                        description: None,
                                                                                                        key: node_scope.clone(),
                                                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                                                            Box::new(::ducktape_view_guest::wire::Node::Text {
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
                                                                                                                key: format!("{}/@text:522", for_scope),
                                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                color: Some(palette.colors[13]),
                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                    monospace: false,
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                },
                                                                                                                width: None,
                                                                                                                align_x: None,
                                                                                                                content: ("Settle →".to_owned()).to_string(),
                                                                                                            }),
                                                                                                        ),
                                                                                                        label: Some(String::from("Settle".to_owned())),
                                                                                                        on_press: if ((!(self.voting).is_empty())) {
                                                                                                            None
                                                                                                        } else {
                                                                                                            Some(
                                                                                                                    ::ducktape_view_guest::slots::message(
                                                                                                                        Message::GovExecute(proposal.id.to_owned()),
                                                                                                                    ),
                                                                                                                )
                                                                                                        },
                                                                                                        width: None,
                                                                                                        height: None,
                                                                                                        padding: Some(
                                                                                                            ::ducktape_view_guest::wire::Edges::all((8.0) as f32),
                                                                                                        ),
                                                                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                                                base: ::ducktape_view_guest::wire::Face {
                                                                                                                    background: None,
                                                                                                                    text: None,
                                                                                                                    border: None,
                                                                                                                },
                                                                                                                hover_background: None,
                                                                                                                pressed_background: None,
                                                                                                                disabled_background: None,
                                                                                                                disabled_text: None,
                                                                                                                disabled_opacity: None,
                                                                                                                focus_ring: None,
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
                                                                                                                background: Some(palette.colors[12]),
                                                                                                                text: Some(palette.colors[13]),
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
                                                                                                            },
                                                                                                            hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                                                background: Some(palette.colors[55]),
                                                                                                                text: Some(palette.colors[13]),
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
                                                                                                            }),
                                                                                                            pressed: None,
                                                                                                            disabled: Some(::ducktape_view_guest::wire::Face {
                                                                                                                background: Some(palette.colors[10]),
                                                                                                                text: Some(palette.colors[11]),
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
                                                                                                            }),
                                                                                                        },
                                                                                                    }
                                                                                                });
                                                                                        }
                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                            max_width: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@layout:490", for_scope),
                                                                                            wrap: None,
                                                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                            spacing: Some((8.0) as f32),
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
                                                                                    key: format!("{}/@layout:448", for_scope),
                                                                                    wrap: None,
                                                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                    spacing: Some((13.0) as f32),
                                                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                        top: (9.0) as f32,
                                                                                        right: (0.0) as f32,
                                                                                        bottom: (0.0) as f32,
                                                                                        left: (0.0) as f32,
                                                                                    }),
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
                                                                            key: format!("{}/@layout:403", for_scope),
                                                                            wrap: None,
                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                            spacing: Some((5.0) as f32),
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
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: node_scope.clone(),
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
                                                }
                                            });
                                    }
                                    if (self.connected
                                        && (!(crate::host::settled_proposals(
                                            ::std::convert::AsRef::as_ref(&(self.rows)),
                                        ))
                                            .is_empty()))
                                    {
                                        children
                                            .push({
                                                let node_scope = format!("{}/settled", node_scope);
                                                {
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
                                                                font: None,
                                                            },
                                                            key: format!("{}/@text:527", node_scope),
                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[73]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: true,
                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: ("RECENTLY FINALIZED".to_owned()).to_string(),
                                                        });
                                                    for (index, proposal) in crate::host::settled_proposals(
                                                            ::std::convert::AsRef::as_ref(&(self.rows)),
                                                        )
                                                        .iter()
                                                        .enumerate()
                                                    {
                                                        let for_scope = format!(
                                                            "{}/@for:1004({})", node_scope, index
                                                        );
                                                        children
                                                            .push(
                                                                self
                                                                    .render_settled_proposal_row_2(
                                                                        palette,
                                                                        format!("{}/SettledProposalRow@1005", for_scope),
                                                                        proposal.clone(),
                                                                    ),
                                                            );
                                                    }
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: node_scope.clone(),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                        spacing: Some((10.0) as f32),
                                                        padding: None,
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        align: None,
                                                        background: None,
                                                        border: None,
                                                        children: children,
                                                    }
                                                }
                                            });
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:287", node_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((16.0) as f32),
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
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:235", node_scope),
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
ducktape_view_guest::export_app!(
    GovernanceView, "Approvals",
    "Every decision this network is being asked to make, and the ones it has settled.",
    ["governance"]
);
