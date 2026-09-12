//! The Members roster as a module-owned view: who may act on this network.
//!
//! The kernel pushes session facts only (`members.props`: connected, admin,
//! dark). The view reads the roster itself through the kernel's
//! `rpc.status` / `rpc.peers` / `rpc.query`, re-reads it on every `rpc.live`
//! hit for the valset plane, and a row opens its record. Pausing an agent
//! and opening a membership ballot leave as `op.submit` — the module
//! message the kernel signs with the seated key; copying a key stays an
//! intent, because the clipboard is an OS door the kernel has not opened.
//! The endpoint, the key and the password never cross: a guest that sees no
//! key cannot leak one.
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
pub(crate) enum MembersFilter {
    All,
    Humans,
    Agents,
    Validators,
}
#[allow(dead_code)]
pub struct MembersView {
    pub(crate) active_palette: AppTheme,
    pub(crate) rows: Vec<crate::host::MemberRow>,
    pub(crate) admin: bool,
    pub(crate) connected: bool,
    pub(crate) connection_serial: i64,
    pub(crate) answered: bool,
    pub(crate) host_error: String,
    pub(crate) filter: MembersFilter,
    pub(crate) selected: String,
    pub(crate) height: i64,
    pub(crate) acting: String,
    pub(crate) sent: bool,
    pub(crate) viewport_width: f64,
    pub(crate) member_width: f64,
}
impl ::std::fmt::Debug for MembersView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("MembersView")
    }
}
#[derive(Clone)]
pub enum Message {
    SessionArrived(crate::host::SessionItem),
    RosterArrived(crate::host::RosterItem),
    ActDone(crate::host::ActItem),
    PickFilter(MembersFilter),
    OpenMember(String),
    CopyKey(String, String),
    MemberResized(f64, f64),
    ViewportChanged(f64, f64),
    SetAgentStatus(String, bool),
    OpenBallot(String, String),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl MembersView {
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
                        ::ducktape_view_guest::wire::Rgba([1.0, 1.0, 1.0, 1.000000]),
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
                        ::ducktape_view_guest::wire::Rgba([1.0, 1.0, 1.0, 1.000000]),
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
                        ::ducktape_view_guest::wire::Rgba([1.0, 1.0, 1.0, 1.000000]),
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
                        ::ducktape_view_guest::wire::Rgba([1.0, 1.0, 1.0, 1.000000]),
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
                        ::ducktape_view_guest::wire::Rgba([1.0, 1.0, 1.0, 1.000000]),
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
impl MembersView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            rows: Vec::new(),
            admin: false,
            connected: false,
            connection_serial: 0,
            answered: false,
            host_error: "".to_owned(),
            filter: MembersFilter::All,
            selected: "".to_owned(),
            height: 0,
            acting: "".to_owned(),
            sent: false,
            viewport_width: 1280.0,
            member_width: 312.0,
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    #[allow(clippy::too_many_arguments)]
    fn restore_state(
        active_palette: AppTheme,
        rows: Vec<crate::host::MemberRow>,
        admin: bool,
        connected: bool,
        connection_serial: i64,
        answered: bool,
        host_error: String,
        filter: MembersFilter,
        selected: String,
        height: i64,
        acting: String,
        sent: bool,
        viewport_width: f64,
        member_width: f64,
    ) -> Self {
        Self {
            active_palette: active_palette,
            rows: rows,
            admin: admin,
            connected: connected,
            connection_serial: connection_serial,
            answered: answered,
            host_error: host_error,
            filter: filter,
            selected: selected,
            height: height,
            acting: acting,
            sent: sent,
            viewport_width: viewport_width,
            member_width: member_width,
        }
    }
    pub(crate) const SNAPSHOT_SCHEMA: &'static str = "c5b4c71dda09d5a068e1b5197b676ac214130791d9b62a6629ac8f67428df93e";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("MembersView"),
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
                    name : String::from("MemberRow"), fields :
                    ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).key))), (String::from("label"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).label))), (String::from("role"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).role))), (String::from("is_this_node"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .is_this_node))), (String::from("is_agent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .is_agent))), (String::from("model"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).model))), (String::from("live"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).live)))]
                    }).collect())), (String::from("admin"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.admin))),
                    (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected))), (String::from("connection_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .connection_serial))), (String::from("answered"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .answered))), (String::from("host_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.host_error))), (String::from("filter"), match & self.filter {
                    MembersFilter::All =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MembersFilter"), fields : vec![(String::from("all"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MembersFilter::Humans =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MembersFilter"), fields : vec![(String::from("humans"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MembersFilter::Agents =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MembersFilter"), fields : vec![(String::from("agents"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    MembersFilter::Validators =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MembersFilter"), fields :
                    vec![(String::from("validators"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("selected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.selected))), (String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self.height))),
                    (String::from("acting"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.acting))), (String::from("sent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.sent))),
                    (String::from("viewport_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .viewport_width))), (String::from("member_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .member_width)))
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
            if name != "MembersView" || fields.len() != 14 {
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
            let rows: Vec<crate::host::MemberRow> = (match value {
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
                            if name != "MemberRow" || fields.len() != 7 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "key" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "label" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "role" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "is_this_node" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "is_agent" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "model" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "live" {
                                return None;
                            }
                            Some(crate::host::MemberRow {
                                key: (match field_0 {
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
                                role: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                is_this_node: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                is_agent: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                model: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                live: (match field_6 {
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
            if name != "filter" {
                return None;
            }
            let filter: MembersFilter = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "MembersFilter" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "all" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MembersFilter::All)
                    }
                    "humans" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MembersFilter::Humans)
                    }
                    "agents" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MembersFilter::Agents)
                    }
                    "validators" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(MembersFilter::Validators)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "selected" {
                return None;
            }
            let selected: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "height" {
                return None;
            }
            let height: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "acting" {
                return None;
            }
            let acting: String = (match value {
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
            if name != "member_width" {
                return None;
            }
            let member_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            Some(
                Self::restore_state(
                    active_palette,
                    rows,
                    admin,
                    connected,
                    connection_serial,
                    answered,
                    host_error,
                    filter,
                    selected,
                    height,
                    acting,
                    sent,
                    viewport_width,
                    member_width,
                ),
            )
        })())
            .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl MembersView {
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::roster(self.connection_serial)
                        .map(move |value| Message::RosterArrived(value)),
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
                let (app, _) = MembersView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
impl MembersView {
    #[allow(clippy::assign_op_pattern)]
    pub(crate) fn update(
        &mut self,
        message: Message,
    ) -> ::ducktape_view_guest::Task<Message> {
        match message {
            Message::SessionArrived(item) => self.on_session_arrived(item),
            Message::RosterArrived(item) => self.on_roster_arrived(item),
            Message::ActDone(item) => self.on_act_done(item),
            Message::PickFilter(next) => self.on_pick_filter(next),
            Message::OpenMember(key) => self.on_open_member(key),
            Message::CopyKey(text, label) => self.on_copy_key(text, label),
            Message::MemberResized(dx, _dy) => self.on_member_resized(dx, _dy),
            Message::ViewportChanged(width, _height) => {
                self.on_viewport_changed(width, _height)
            }
            Message::SetAgentStatus(agent_id, paused) => {
                self.on_set_agent_status(agent_id, paused)
            }
            Message::OpenBallot(action, key) => self.on_open_ballot(action, key),
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
    fn on_roster_arrived(
        &mut self,
        item: crate::host::RosterItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.host_error = item.error.to_owned();
            self.answered = true;
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            self.rows = item.rows.clone();
            self.height = item.height;
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_act_done(
        &mut self,
        item: crate::host::ActItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.acting = "".to_owned();
            self.host_error = item.error.to_owned();
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_pick_filter(
        &mut self,
        next: MembersFilter,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.filter = next.clone();
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_member(&mut self, key: String) -> ::ducktape_view_guest::Task<Message> {
        {
            self.selected = key.to_owned();
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_copy_key(
        &mut self,
        text: String,
        label: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.sent = (crate::host::copy(
                ::std::convert::AsRef::as_ref(&(text)),
                ::std::convert::AsRef::as_ref(&(label)),
            ));
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_member_resized(
        &mut self,
        dx: f64,
        _dy: f64,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.member_width = crate::host::member_width_after_delta(
                self.member_width,
                (-dx),
                self.viewport_width,
            );
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_viewport_changed(
        &mut self,
        width: f64,
        _height: f64,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.viewport_width = width;
            self.member_width = crate::host::member_width_after_delta(
                self.member_width,
                0.0,
                width,
            );
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_set_agent_status(
        &mut self,
        agent_id: String,
        paused: bool,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((!self.connected) || (!(self.acting).is_empty())) {
                return ::ducktape_view_guest::Task::none();
            }
            self.acting = agent_id.to_owned();
            let _sent = (crate::host::agent_status(
                ::std::convert::AsRef::as_ref(&(agent_id)),
                paused,
            ));
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_ballot(
        &mut self,
        action: String,
        key: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            if (((!self.connected) || (!self.admin)) || (!(self.acting).is_empty())) {
                return ::ducktape_view_guest::Task::none();
            }
            self.acting = key.to_owned();
            let _sent = (crate::host::propose(
                ::std::convert::AsRef::as_ref(&(action)),
                ::std::convert::AsRef::as_ref(&(key)),
                self.height,
            ));
            ::ducktape_view_guest::Task::none()
        }
    }
}
impl MembersView {
    pub(crate) fn view(&self) -> ::ducktape_view_guest::wire::Node {
        let palette = self.palette();
        {
            let node_scope = format!("{}/root", "MembersView");
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
                        key: format!("{}/@container:127", node_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                        padding: None,
                        align_x: None,
                        align_y: None,
                        background: (Some(palette.colors[2]))
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
                    .push(::ducktape_view_guest::wire::Node::Sensor {
                        key: format!("{}/@sensor:129", node_scope),
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
                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                        }),
                    });
                children
                    .push({
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
                                        key: format!("{}/@container:133", node_scope),
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
                                                        content: ("Members".to_owned()).to_string(),
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
                                                        content: (crate::host::members_summary(
                                                            self.connected,
                                                            ::std::convert::AsRef::as_ref(&(self.rows)),
                                                        ))
                                                            .to_string(),
                                                    }
                                                });
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Space {
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:138", node_scope),
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
                                if self.connected {
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
                                            key: format!("{}/@container:158", node_scope),
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                top: (12.0) as f32,
                                                right: (22.0) as f32,
                                                bottom: (12.0) as f32,
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
                                                if (self.filter == MembersFilter::All) {
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:171", node_scope),
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
                                                                    key: format!("{}/@container:172", node_scope),
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
                                                                                key: format!("{}/@text:181", node_scope),
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
                                                                                    wrapping: None,
                                                                                    tracking: 0.0f32,
                                                                                    font: None,
                                                                                },
                                                                                key: format!("{}/@text:182", node_scope),
                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: true,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (((self.rows).len() as i64)).to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:180", node_scope),
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
                                                                }),
                                                            ),
                                                            label: Some(String::from("Show every member".to_owned())),
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::PickFilter(MembersFilter::All),
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
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                }
                                                if (self.filter != MembersFilter::All) {
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:191", node_scope),
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
                                                                    key: format!("{}/@container:192", node_scope),
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
                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[4]),
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
                                                                                    wrapping: None,
                                                                                    tracking: 0.0f32,
                                                                                    font: None,
                                                                                },
                                                                                key: format!("{}/@text:202", node_scope),
                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: true,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (((self.rows).len() as i64)).to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:200", node_scope),
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
                                                                }),
                                                            ),
                                                            label: Some(String::from("Show every member".to_owned())),
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::PickFilter(MembersFilter::All),
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
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                }
                                                if (self.filter == MembersFilter::Humans) {
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:211", node_scope),
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
                                                                    key: format!("{}/@container:212", node_scope),
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
                                                                                key: format!("{}/@text:221", node_scope),
                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[9]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("Humans".to_owned()).to_string(),
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
                                                                                key: format!("{}/@text:222", node_scope),
                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: true,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (((crate::host::filter_members(
                                                                                    ::std::convert::AsRef::as_ref(&(self.rows)),
                                                                                    MembersFilter::Humans,
                                                                                ))
                                                                                    .len() as i64))
                                                                                    .to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:220", node_scope),
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
                                                                }),
                                                            ),
                                                            label: Some(String::from("Show people only".to_owned())),
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::PickFilter(MembersFilter::Humans),
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
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                }
                                                if (self.filter != MembersFilter::Humans) {
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:231", node_scope),
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
                                                                    key: format!("{}/@container:232", node_scope),
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
                                                                                key: format!("{}/@text:241", node_scope),
                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[4]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("Humans".to_owned()).to_string(),
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
                                                                                key: format!("{}/@text:242", node_scope),
                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: true,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (((crate::host::filter_members(
                                                                                    ::std::convert::AsRef::as_ref(&(self.rows)),
                                                                                    MembersFilter::Humans,
                                                                                ))
                                                                                    .len() as i64))
                                                                                    .to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:240", node_scope),
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
                                                                }),
                                                            ),
                                                            label: Some(String::from("Show people only".to_owned())),
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::PickFilter(MembersFilter::Humans),
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
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                }
                                                if (self.filter == MembersFilter::Agents) {
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:251", node_scope),
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
                                                                    key: format!("{}/@container:252", node_scope),
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
                                                                                key: format!("{}/@text:261", node_scope),
                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[9]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("Agents".to_owned()).to_string(),
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
                                                                                key: format!("{}/@text:262", node_scope),
                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: true,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (((crate::host::filter_members(
                                                                                    ::std::convert::AsRef::as_ref(&(self.rows)),
                                                                                    MembersFilter::Agents,
                                                                                ))
                                                                                    .len() as i64))
                                                                                    .to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:260", node_scope),
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
                                                                }),
                                                            ),
                                                            label: Some(String::from("Show agents only".to_owned())),
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::PickFilter(MembersFilter::Agents),
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
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                }
                                                if (self.filter != MembersFilter::Agents) {
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:271", node_scope),
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
                                                                    key: format!("{}/@container:272", node_scope),
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
                                                                                key: format!("{}/@text:281", node_scope),
                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[4]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("Agents".to_owned()).to_string(),
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
                                                                                key: format!("{}/@text:282", node_scope),
                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: true,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (((crate::host::filter_members(
                                                                                    ::std::convert::AsRef::as_ref(&(self.rows)),
                                                                                    MembersFilter::Agents,
                                                                                ))
                                                                                    .len() as i64))
                                                                                    .to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:280", node_scope),
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
                                                                }),
                                                            ),
                                                            label: Some(String::from("Show agents only".to_owned())),
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::PickFilter(MembersFilter::Agents),
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
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                }
                                                if (self.filter == MembersFilter::Validators) {
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:291", node_scope),
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
                                                                    key: format!("{}/@container:292", node_scope),
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
                                                                                key: format!("{}/@text:301", node_scope),
                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[9]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("Validators".to_owned()).to_string(),
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
                                                                                key: format!("{}/@text:302", node_scope),
                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: true,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (((crate::host::filter_members(
                                                                                    ::std::convert::AsRef::as_ref(&(self.rows)),
                                                                                    MembersFilter::Validators,
                                                                                ))
                                                                                    .len() as i64))
                                                                                    .to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:300", node_scope),
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
                                                                }),
                                                            ),
                                                            label: Some(
                                                                String::from("Show validators only".to_owned()),
                                                            ),
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::PickFilter(MembersFilter::Validators),
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
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                }
                                                if (self.filter != MembersFilter::Validators) {
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:311", node_scope),
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
                                                                    key: format!("{}/@container:312", node_scope),
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
                                                                                key: format!("{}/@text:321", node_scope),
                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[4]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("Validators".to_owned()).to_string(),
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
                                                                                key: format!("{}/@text:322", node_scope),
                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: true,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (((crate::host::filter_members(
                                                                                    ::std::convert::AsRef::as_ref(&(self.rows)),
                                                                                    MembersFilter::Validators,
                                                                                ))
                                                                                    .len() as i64))
                                                                                    .to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:320", node_scope),
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
                                                                }),
                                                            ),
                                                            label: Some(
                                                                String::from("Show validators only".to_owned()),
                                                            ),
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::PickFilter(MembersFilter::Validators),
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
                                                                pressed: None,
                                                                disabled: None,
                                                            },
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
                                                    key: format!("{}/@layout:165", node_scope),
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
                                            }),
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
                                        max_width: None,
                                        max_height: None,
                                        clip: false,
                                        key: format!("{}/@container:331", node_scope),
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
                                                            key: format!("{}/@container:354", node_scope),
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
                                                                key: format!("{}/@text:364", node_scope),
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
                                                            key: format!("{}/@text:365", node_scope),
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
                                                            key: format!("{}/@text:370", node_scope),
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
                                                        key: format!("{}/@layout:349", node_scope),
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
                                if ((self.connected
                                    && (crate::host::filter_members(
                                        ::std::convert::AsRef::as_ref(&(self.rows)),
                                        self.filter.clone(),
                                    ))
                                        .is_empty()) && self.answered)
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
                                            key: format!("{}/@container:375", node_scope),
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                top: (22.0) as f32,
                                                right: (22.0) as f32,
                                                bottom: (22.0) as f32,
                                                left: (22.0) as f32,
                                            }),
                                            align_x: None,
                                            align_y: None,
                                            background: (None)
                                                .map(::ducktape_view_guest::wire::Background::Color),
                                            border: None,
                                            snap: None,
                                            content: Box::new({
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
                                                        key: format!("{}/@text:388", node_scope),
                                                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[71]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("No members here yet — validators, residents and registered agents appear as they join."
                                                            .to_owned())
                                                            .to_string(),
                                                    }),
                                                }
                                            }),
                                        });
                                }
                                if (self.connected
                                    && (!(crate::host::filter_members(
                                        ::std::convert::AsRef::as_ref(&(self.rows)),
                                        self.filter.clone(),
                                    ))
                                        .is_empty()))
                                {
                                    children
                                        .push({
                                            let node_scope = format!("{}/members-body", node_scope);
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
                                                    for (index, member) in crate::host::filter_members(
                                                            ::std::convert::AsRef::as_ref(&(self.rows)),
                                                            self.filter.clone(),
                                                        )
                                                        .iter()
                                                        .enumerate()
                                                    {
                                                        let for_scope = format!(
                                                            "{}/@for:878({})", node_scope, index
                                                        );
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                if (member.key == self.selected) {
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
                                                                            key: format!("{}/@container:414", for_scope),
                                                                            width: Some(
                                                                                ::ducktape_view_guest::wire::Length::Fixed((3.0) as f32),
                                                                            ),
                                                                            height: Some(
                                                                                ::ducktape_view_guest::wire::Length::Fixed((44.0) as f32),
                                                                            ),
                                                                            padding: None,
                                                                            align_x: None,
                                                                            align_y: None,
                                                                            background: (Some(palette.colors[7]))
                                                                                .map(::ducktape_view_guest::wire::Background::Color),
                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                color: None,
                                                                                width: None,
                                                                                radius: Some([
                                                                                    ((1.5) as f32).max(0.0).min(f32::MAX),
                                                                                    ((1.5) as f32).max(0.0).min(f32::MAX),
                                                                                    ((1.5) as f32).max(0.0).min(f32::MAX),
                                                                                    ((1.5) as f32).max(0.0).min(f32::MAX),
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
                                                                if (member.key != self.selected) {
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
                                                                            key: format!("{}/@container:422", for_scope),
                                                                            width: Some(
                                                                                ::ducktape_view_guest::wire::Length::Fixed((3.0) as f32),
                                                                            ),
                                                                            height: Some(
                                                                                ::ducktape_view_guest::wire::Length::Fixed((44.0) as f32),
                                                                            ),
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
                                                                        });
                                                                }
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: None,
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:425", for_scope),
                                                                        content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                            Box::new({
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
                                                                                        key: format!("{}/@container:431", for_scope),
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        height: None,
                                                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                            top: (12.0) as f32,
                                                                                            right: (14.0) as f32,
                                                                                            bottom: (12.0) as f32,
                                                                                            left: (14.0) as f32,
                                                                                        }),
                                                                                        align_x: None,
                                                                                        align_y: None,
                                                                                        background: (None)
                                                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                                                        border: None,
                                                                                        snap: None,
                                                                                        content: Box::new({
                                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                            if member.is_agent {
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
                                                                                                        key: format!("{}/@container:444", for_scope),
                                                                                                        width: Some(
                                                                                                            ::ducktape_view_guest::wire::Length::Fixed((32.0) as f32),
                                                                                                        ),
                                                                                                        height: Some(
                                                                                                            ::ducktape_view_guest::wire::Length::Fixed((32.0) as f32),
                                                                                                        ),
                                                                                                        padding: None,
                                                                                                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                        background: (Some(palette.colors[7]))
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
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:452", for_scope),
                                                                                                            size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[38]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (crate::host::initials_of(
                                                                                                                ::std::convert::AsRef::as_ref(&(member.label)),
                                                                                                            ))
                                                                                                                .to_string(),
                                                                                                        }),
                                                                                                    });
                                                                                            }
                                                                                            if (!member.is_agent) {
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
                                                                                                        key: format!("{}/@container:459", for_scope),
                                                                                                        width: Some(
                                                                                                            ::ducktape_view_guest::wire::Length::Fixed((32.0) as f32),
                                                                                                        ),
                                                                                                        height: Some(
                                                                                                            ::ducktape_view_guest::wire::Length::Fixed((32.0) as f32),
                                                                                                        ),
                                                                                                        padding: None,
                                                                                                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                                        align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                                        background: (Some(palette.colors[35]))
                                                                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                                                            color: None,
                                                                                                            width: None,
                                                                                                            radius: Some([
                                                                                                                ((16.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                ((16.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                ((16.0) as f32).max(0.0).min(f32::MAX),
                                                                                                                ((16.0) as f32).max(0.0).min(f32::MAX),
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
                                                                                                            key: format!("{}/@text:467", for_scope),
                                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[36]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: false,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (crate::host::initial_of(
                                                                                                                ::std::convert::AsRef::as_ref(&(member.label)),
                                                                                                            ))
                                                                                                                .to_string(),
                                                                                                        }),
                                                                                                    });
                                                                                            }
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
                                                                                                                    key: format!("{}/@text:470", for_scope),
                                                                                                                    size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                    color: Some(palette.colors[4]),
                                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                                        monospace: false,
                                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                    },
                                                                                                                    width: None,
                                                                                                                    align_x: None,
                                                                                                                    content: (member.label.to_owned()).to_string(),
                                                                                                                });
                                                                                                            if member.is_this_node {
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
                                                                                                                        key: format!("{}/@text:476", for_scope),
                                                                                                                        size: Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                        color: Some(palette.colors[71]),
                                                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                                                            monospace: false,
                                                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                        },
                                                                                                                        width: None,
                                                                                                                        align_x: None,
                                                                                                                        content: ("this node".to_owned()).to_string(),
                                                                                                                    });
                                                                                                            }
                                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                                max_width: None,
                                                                                                                clip: false,
                                                                                                                key: format!("{}/@layout:469", for_scope),
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
                                                                                                            key: format!("{}/@text:477", for_scope),
                                                                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[72]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (member.key.to_owned()).to_string(),
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:468", for_scope),
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
                                                                                            if (member.role == "validator") {
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
                                                                                                        key: format!("{}/@container:483", for_scope),
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
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:489", for_scope),
                                                                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[9]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: ("VALIDATOR".to_owned()).to_string(),
                                                                                                        }),
                                                                                                    });
                                                                                            }
                                                                                            if (member.role == "agent") {
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
                                                                                                        key: format!("{}/@container:496", for_scope),
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
                                                                                                        background: (Some(palette.colors[18]))
                                                                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                                                            color: Some(palette.colors[19]),
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
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:504", for_scope),
                                                                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[16]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: ("AGENT".to_owned()).to_string(),
                                                                                                        }),
                                                                                                    });
                                                                                            }
                                                                                            if (member.role == "resident") {
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
                                                                                                        key: format!("{}/@container:511", for_scope),
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
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:519", for_scope),
                                                                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[41]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: ("RESIDENT".to_owned()).to_string(),
                                                                                                        }),
                                                                                                    });
                                                                                            }
                                                                                            if (((member.role != "validator")
                                                                                                && (member.role != "agent")) && (member.role != "resident"))
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
                                                                                                        key: format!("{}/@container:526", for_scope),
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
                                                                                                            key: format!("{}/@text:534", for_scope),
                                                                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[30]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (member.role.to_owned()).to_string(),
                                                                                                        }),
                                                                                                    });
                                                                                            }
                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                max_width: None,
                                                                                                clip: false,
                                                                                                key: format!("{}/@layout:438", for_scope),
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
                                                                                        key: format!("{}/@container:540", for_scope),
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        height: Some(
                                                                                            ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                                                        ),
                                                                                        padding: None,
                                                                                        align_x: None,
                                                                                        align_y: None,
                                                                                        background: (Some(palette.colors[6]))
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
                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                    max_width: None,
                                                                                    clip: false,
                                                                                    key: format!("{}/@layout:430", for_scope),
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
                                                                        ),
                                                                        label: Some(String::from(member.label.to_owned())),
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(
                                                                                Message::OpenMember(member.key.to_owned()),
                                                                            ),
                                                                        ),
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        height: None,
                                                                        padding: Some(
                                                                            ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
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
                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                    ]),
                                                                                }),
                                                                            },
                                                                            hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[57]),
                                                                                text: Some(palette.colors[4]),
                                                                                border: None,
                                                                            }),
                                                                            pressed: None,
                                                                            disabled: None,
                                                                        },
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:412", for_scope),
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
                                                    }
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:398", node_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                        spacing: Some((1.0) as f32),
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (6.0) as f32,
                                                            right: (12.0) as f32,
                                                            bottom: (6.0) as f32,
                                                            left: (12.0) as f32,
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
                                }
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:132", node_scope),
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
                            });
                        if (self.connected && (!(self.selected).is_empty())) {
                            for (index, member) in self.rows.iter().enumerate() {
                                let for_scope = format!(
                                    "{}/@for:1020({})", node_scope, index
                                );
                                if (member.key == self.selected) {
                                    children
                                        .push({
                                            let node_scope = format!("{}/member-resize", node_scope);
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
                                                            let route = move |delta: (f64, f64)| Message::MemberResized(
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
                                                    let node_scope = format!("{}/member-divider", node_scope);
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
                                                        align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                        align_y: None,
                                                        background: (Some(palette.colors[54]))
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(::ducktape_view_guest::wire::Node::Container {
                                                            shadow: ::ducktape_view_guest::wire::Shadow {
                                                                color: None,
                                                                x: None,
                                                                y: None,
                                                                blur: None,
                                                            },
                                                            max_width: None,
                                                            max_height: None,
                                                            clip: false,
                                                            key: format!("{}/@container:560", for_scope),
                                                            width: Some(
                                                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                                                            ),
                                                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            padding: None,
                                                            align_x: None,
                                                            align_y: None,
                                                            background: (Some(palette.colors[60]))
                                                                .map(::ducktape_view_guest::wire::Background::Color),
                                                            border: None,
                                                            snap: None,
                                                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                                                width: Some(
                                                                    ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                                                                ),
                                                                height: Some(
                                                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                                                ),
                                                            }),
                                                        }),
                                                    }
                                                }),
                                            }
                                        });
                                    children
                                        .push({
                                            let node_scope = format!("{}/member", node_scope);
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
                                                        (self.member_width) as f32,
                                                    ),
                                                ),
                                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                padding: None,
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[54]))
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
                                                            key: format!("{}/@container:568", for_scope),
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: Some(
                                                                ::ducktape_view_guest::wire::Length::Fixed((56.0) as f32),
                                                            ),
                                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                                top: (0.0) as f32,
                                                                right: (16.0) as f32,
                                                                bottom: (0.0) as f32,
                                                                left: (16.0) as f32,
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
                                                                        key: format!("{}/@text:579", for_scope),
                                                                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[4]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        align_x: None,
                                                                        content: ("Member".to_owned()).to_string(),
                                                                    });
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: None,
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:584", for_scope),
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
                                                                                key: format!("{}/@text:590", for_scope),
                                                                                size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("×".to_owned()).to_string(),
                                                                            }),
                                                                        ),
                                                                        label: Some(String::from("Close member".to_owned())),
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(
                                                                                Message::OpenMember("".to_owned()),
                                                                            ),
                                                                        ),
                                                                        width: Some(
                                                                            ::ducktape_view_guest::wire::Length::Fixed((24.0) as f32),
                                                                        ),
                                                                        height: Some(
                                                                            ::ducktape_view_guest::wire::Length::Fixed((24.0) as f32),
                                                                        ),
                                                                        padding: Some(
                                                                            ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
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
                                                                                background: Some(
                                                                                    ::ducktape_view_guest::wire::Rgba([
                                                                                        0.0 / 255.0,
                                                                                        0.0 / 255.0,
                                                                                        0.0 / 255.0,
                                                                                        0.000000,
                                                                                    ]),
                                                                                ),
                                                                                text: None,
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
                                                                            },
                                                                            hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[60]),
                                                                                text: None,
                                                                                border: None,
                                                                            }),
                                                                            pressed: None,
                                                                            disabled: None,
                                                                        },
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:573", for_scope),
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
                                                            }),
                                                        });
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Scroll {
                                                            on_scroll: None,
                                                            virtual_rows: false,
                                                            key: format!("{}/@layout:593", for_scope),
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
                                                                        if member.is_agent {
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
                                                                                    key: format!("{}/@container:612", for_scope),
                                                                                    width: Some(
                                                                                        ::ducktape_view_guest::wire::Length::Fixed((54.0) as f32),
                                                                                    ),
                                                                                    height: Some(
                                                                                        ::ducktape_view_guest::wire::Length::Fixed((54.0) as f32),
                                                                                    ),
                                                                                    padding: None,
                                                                                    align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                    align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                    background: (Some(palette.colors[7]))
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
                                                                                            wrapping: None,
                                                                                            tracking: 0.0f32,
                                                                                            font: None,
                                                                                        },
                                                                                        key: format!("{}/@text:620", for_scope),
                                                                                        size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[38]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: true,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: (crate::host::initials_of(
                                                                                            ::std::convert::AsRef::as_ref(&(member.label)),
                                                                                        ))
                                                                                            .to_string(),
                                                                                    }),
                                                                                });
                                                                        }
                                                                        if (!member.is_agent) {
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
                                                                                    key: format!("{}/@container:627", for_scope),
                                                                                    width: Some(
                                                                                        ::ducktape_view_guest::wire::Length::Fixed((54.0) as f32),
                                                                                    ),
                                                                                    height: Some(
                                                                                        ::ducktape_view_guest::wire::Length::Fixed((54.0) as f32),
                                                                                    ),
                                                                                    padding: None,
                                                                                    align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                    align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                                                                                    background: (Some(palette.colors[35]))
                                                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: None,
                                                                                        width: None,
                                                                                        radius: Some([
                                                                                            ((27.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ((27.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ((27.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ((27.0) as f32).max(0.0).min(f32::MAX),
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
                                                                                        key: format!("{}/@text:635", for_scope),
                                                                                        size: Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[36]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: (crate::host::initial_of(
                                                                                            ::std::convert::AsRef::as_ref(&(member.label)),
                                                                                        ))
                                                                                            .to_string(),
                                                                                    }),
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
                                                                                max_width: None,
                                                                                max_height: None,
                                                                                clip: false,
                                                                                key: format!("{}/@container:636", for_scope),
                                                                                width: None,
                                                                                height: None,
                                                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                    top: (11.0) as f32,
                                                                                    right: (0.0) as f32,
                                                                                    bottom: (0.0) as f32,
                                                                                    left: (0.0) as f32,
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
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:637", for_scope),
                                                                                    size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[4]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: (member.label.to_owned()).to_string(),
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
                                                                                key: format!("{}/@container:645", for_scope),
                                                                                width: None,
                                                                                height: None,
                                                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                    top: (5.0) as f32,
                                                                                    right: (0.0) as f32,
                                                                                    bottom: (0.0) as f32,
                                                                                    left: (0.0) as f32,
                                                                                }),
                                                                                align_x: None,
                                                                                align_y: None,
                                                                                background: (None)
                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                border: None,
                                                                                snap: None,
                                                                                content: Box::new({
                                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                    if member.live {
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
                                                                                                key: format!("{}/@container:648", for_scope),
                                                                                                width: Some(
                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((7.0) as f32),
                                                                                                ),
                                                                                                height: Some(
                                                                                                    ::ducktape_view_guest::wire::Length::Fixed((7.0) as f32),
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
                                                                                    if (!member.live) {
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
                                                                                                key: format!("{}/@container:656", for_scope),
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
                                                                                    if (member.is_agent && member.live) {
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
                                                                                                key: format!("{}/@text:664", for_scope),
                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[41]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("active".to_owned()).to_string(),
                                                                                            });
                                                                                    }
                                                                                    if (member.is_agent && (!member.live)) {
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
                                                                                                key: format!("{}/@text:666", for_scope),
                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[41]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("paused".to_owned()).to_string(),
                                                                                            });
                                                                                    }
                                                                                    if ((!member.is_agent) && member.live) {
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
                                                                                                key: format!("{}/@text:668", for_scope),
                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[41]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("live".to_owned()).to_string(),
                                                                                            });
                                                                                    }
                                                                                    if ((!member.is_agent) && (!member.live)) {
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
                                                                                                key: format!("{}/@text:670", for_scope),
                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[41]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("offline".to_owned()).to_string(),
                                                                                            });
                                                                                    }
                                                                                    if (member.role == "validator") {
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
                                                                                                key: format!("{}/@container:672", for_scope),
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
                                                                                                        wrapping: None,
                                                                                                        tracking: 0.0f32,
                                                                                                        font: None,
                                                                                                    },
                                                                                                    key: format!("{}/@text:678", for_scope),
                                                                                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[9]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: true,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: ("VALIDATOR".to_owned()).to_string(),
                                                                                                }),
                                                                                            });
                                                                                    }
                                                                                    if (member.role == "agent") {
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
                                                                                                key: format!("{}/@container:685", for_scope),
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
                                                                                                background: (Some(palette.colors[18]))
                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[19]),
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
                                                                                                        wrapping: None,
                                                                                                        tracking: 0.0f32,
                                                                                                        font: None,
                                                                                                    },
                                                                                                    key: format!("{}/@text:693", for_scope),
                                                                                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[16]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: true,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: ("AGENT".to_owned()).to_string(),
                                                                                                }),
                                                                                            });
                                                                                    }
                                                                                    if (member.role == "resident") {
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
                                                                                                key: format!("{}/@container:700", for_scope),
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
                                                                                                        line_height: None,
                                                                                                        shaping: None,
                                                                                                        wrapping: None,
                                                                                                        tracking: 0.0f32,
                                                                                                        font: None,
                                                                                                    },
                                                                                                    key: format!("{}/@text:708", for_scope),
                                                                                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[41]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: true,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: ("RESIDENT".to_owned()).to_string(),
                                                                                                }),
                                                                                            });
                                                                                    }
                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                        max_width: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@layout:646", for_scope),
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
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:606", for_scope),
                                                                            wrap: None,
                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                            spacing: Some((0.0) as f32),
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
                                                                                key: format!("{}/@container:721", for_scope),
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
                                                                                    if member.is_agent {
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
                                                                                                key: format!("{}/@text:736", for_scope),
                                                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[71]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: true,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("agent id".to_owned()).to_string(),
                                                                                            });
                                                                                    }
                                                                                    if (!member.is_agent) {
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
                                                                                                key: format!("{}/@text:742", for_scope),
                                                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[71]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: true,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("public key".to_owned()).to_string(),
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
                                                                                                font: None,
                                                                                            },
                                                                                            key: format!("{}/@text:747", for_scope),
                                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[13]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: true,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                            align_x: None,
                                                                                            content: (member.key.to_owned()).to_string(),
                                                                                        });
                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                        max_width: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@layout:730", for_scope),
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
                                                                            });
                                                                        if (!(member.model).is_empty()) {
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
                                                                                    key: format!("{}/@container:754", for_scope),
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
                                                                                                    wrapping: None,
                                                                                                    tracking: 0.0f32,
                                                                                                    font: None,
                                                                                                },
                                                                                                key: format!("{}/@text:768", for_scope),
                                                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[71]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: true,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("capability".to_owned()).to_string(),
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
                                                                                                key: format!("{}/@text:773", for_scope),
                                                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[13]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: true,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                align_x: None,
                                                                                                content: (member.model.to_owned()).to_string(),
                                                                                            });
                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                            max_width: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@layout:763", for_scope),
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
                                                                                });
                                                                        }
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:714", for_scope),
                                                                            wrap: None,
                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                            spacing: Some((8.0) as f32),
                                                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                top: (18.0) as f32,
                                                                                right: (0.0) as f32,
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
                                                                    });
                                                                children
                                                                    .push({
                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                        if member.is_this_node {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:787", for_scope),
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
                                                                                            key: format!("{}/@text:792", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[15]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: false,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                            align_x: None,
                                                                                            content: ("Copy this node's key".to_owned()).to_string(),
                                                                                        }),
                                                                                    ),
                                                                                    label: Some(
                                                                                        String::from("Copy this node's key".to_owned()),
                                                                                    ),
                                                                                    on_press: Some(
                                                                                        ::ducktape_view_guest::slots::message(
                                                                                            Message::CopyKey(
                                                                                                member.key.to_owned(),
                                                                                                "Node key copied".to_owned(),
                                                                                            ),
                                                                                        ),
                                                                                    ),
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((10.0) as f32),
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
                                                                                            text: None,
                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                color: Some(palette.colors[40]),
                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                radius: Some([
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ]),
                                                                                            }),
                                                                                        },
                                                                                        hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                            background: Some(palette.colors[57]),
                                                                                            text: None,
                                                                                            border: None,
                                                                                        }),
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                        }
                                                                        if (member.is_agent && member.live) {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:802", for_scope),
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
                                                                                            key: format!("{}/@text:807", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[15]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: false,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                            align_x: None,
                                                                                            content: ("Pause agent".to_owned()).to_string(),
                                                                                        }),
                                                                                    ),
                                                                                    label: Some(String::from("Pause agent".to_owned())),
                                                                                    on_press: Some(
                                                                                        ::ducktape_view_guest::slots::message(
                                                                                            Message::SetAgentStatus(member.key.to_owned(), true),
                                                                                        ),
                                                                                    ),
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((10.0) as f32),
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
                                                                                            text: None,
                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                color: Some(palette.colors[40]),
                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                radius: Some([
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ]),
                                                                                            }),
                                                                                        },
                                                                                        hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                            background: Some(palette.colors[57]),
                                                                                            text: None,
                                                                                            border: None,
                                                                                        }),
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                        }
                                                                        if (member.is_agent && (!member.live)) {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:815", for_scope),
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
                                                                                            key: format!("{}/@text:820", for_scope),
                                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[15]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: false,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                            align_x: None,
                                                                                            content: ("Resume agent".to_owned()).to_string(),
                                                                                        }),
                                                                                    ),
                                                                                    label: Some(String::from("Resume agent".to_owned())),
                                                                                    on_press: Some(
                                                                                        ::ducktape_view_guest::slots::message(
                                                                                            Message::SetAgentStatus(member.key.to_owned(), false),
                                                                                        ),
                                                                                    ),
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((10.0) as f32),
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
                                                                                            text: None,
                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                color: Some(palette.colors[40]),
                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                radius: Some([
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ]),
                                                                                            }),
                                                                                        },
                                                                                        hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                            background: Some(palette.colors[57]),
                                                                                            text: None,
                                                                                            border: None,
                                                                                        }),
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                        }
                                                                        if member.is_agent {
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
                                                                                    key: format!("{}/@container:828", for_scope),
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
                                                                                                key: format!("{}/@text:838", for_scope),
                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[4]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("Pause and resume are owner-gated writes."
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
                                                                                                key: format!("{}/@text:839", for_scope),
                                                                                                size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[70]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("The model accepts changes from its program account or current controller."
                                                                                                    .to_owned())
                                                                                                    .to_string(),
                                                                                            });
                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                            max_width: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@layout:837", for_scope),
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
                                                                        if ((self.admin && (!member.is_agent))
                                                                            && (member.role == "resident"))
                                                                        {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:846", for_scope),
                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                                        Box::new({
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
                                                                                                    key: format!("{}/@text:856", for_scope),
                                                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[15]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: false,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                    },
                                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                    align_x: None,
                                                                                                    content: ("Promote to validator".to_owned()).to_string(),
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
                                                                                                    key: format!("{}/@text:861", for_scope),
                                                                                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[73]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: true,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: ("needs quorum".to_owned()).to_string(),
                                                                                                });
                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                max_width: None,
                                                                                                clip: false,
                                                                                                key: format!("{}/@layout:851", for_scope),
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
                                                                                    ),
                                                                                    label: Some(
                                                                                        String::from("Promote to validator".to_owned()),
                                                                                    ),
                                                                                    on_press: Some(
                                                                                        ::ducktape_view_guest::slots::message(
                                                                                            Message::OpenBallot(
                                                                                                "add_validator".to_owned(),
                                                                                                member.key.to_owned(),
                                                                                            ),
                                                                                        ),
                                                                                    ),
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((10.0) as f32),
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
                                                                                            text: None,
                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                color: Some(palette.colors[40]),
                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                radius: Some([
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ]),
                                                                                            }),
                                                                                        },
                                                                                        hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                            background: Some(palette.colors[57]),
                                                                                            text: None,
                                                                                            border: None,
                                                                                        }),
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                        }
                                                                        if (((self.admin && (!member.is_agent))
                                                                            && (member.role == "validator")) && (!member.is_this_node))
                                                                        {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:870", for_scope),
                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                                        Box::new({
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
                                                                                                    key: format!("{}/@text:880", for_scope),
                                                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[80]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: false,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                    },
                                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                    align_x: None,
                                                                                                    content: ("Remove from the validator set".to_owned())
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
                                                                                                        font: None,
                                                                                                    },
                                                                                                    key: format!("{}/@text:885", for_scope),
                                                                                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[73]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: true,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: ("needs quorum".to_owned()).to_string(),
                                                                                                });
                                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                                max_width: None,
                                                                                                clip: false,
                                                                                                key: format!("{}/@layout:875", for_scope),
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
                                                                                    ),
                                                                                    label: Some(
                                                                                        String::from("Remove from the validator set".to_owned()),
                                                                                    ),
                                                                                    on_press: Some(
                                                                                        ::ducktape_view_guest::slots::message(
                                                                                            Message::OpenBallot(
                                                                                                "remove_validator".to_owned(),
                                                                                                member.key.to_owned(),
                                                                                            ),
                                                                                        ),
                                                                                    ),
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((10.0) as f32),
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
                                                                                            text: None,
                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                color: Some(palette.colors[82]),
                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                radius: Some([
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ]),
                                                                                            }),
                                                                                        },
                                                                                        hovered: Some(::ducktape_view_guest::wire::Face {
                                                                                            background: Some(palette.colors[57]),
                                                                                            text: None,
                                                                                            border: None,
                                                                                        }),
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                        }
                                                                        if (((!self.admin) && (!member.is_this_node))
                                                                            && (!member.is_agent))
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
                                                                                    key: format!("{}/@container:894", for_scope),
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
                                                                                                key: format!("{}/@text:904", for_scope),
                                                                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[4]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("Only a validator node may open a membership proposal."
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
                                                                                                key: format!("{}/@text:908", for_scope),
                                                                                                size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[70]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: ("This node holds no quorum seat, so the network refuses the write."
                                                                                                    .to_owned())
                                                                                                    .to_string(),
                                                                                            });
                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                            max_width: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@layout:903", for_scope),
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
                                                                            key: format!("{}/@layout:779", for_scope),
                                                                            wrap: None,
                                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                            spacing: Some((8.0) as f32),
                                                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                top: (16.0) as f32,
                                                                                right: (0.0) as f32,
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
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:598", for_scope),
                                                                    wrap: None,
                                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                    spacing: Some((0.0) as f32),
                                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                                        top: (18.0) as f32,
                                                                        right: (16.0) as f32,
                                                                        bottom: (18.0) as f32,
                                                                        left: (16.0) as f32,
                                                                    }),
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
                                                        key: format!("{}/@layout:567", for_scope),
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
                                        });
                                }
                            }
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:131", node_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: None,
                            padding: None,
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(::ducktape_view_guest::wire::Length::Fill),
                            align: None,
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                ::ducktape_view_guest::wire::Node::Stack {
                    key: node_scope.clone(),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                    padding: None,
                    background: None,
                    border: None,
                    clip: false,
                    under: 0u32,
                    children: children,
                }
            }
        }
    }
}
ducktape_view_guest::export_app!(
    MembersView, "Members",
    "Who may act on this network: validators, residents and registered agents.",
    ["members"]
);
