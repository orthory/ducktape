//! The Agents register as a view on the kernel contract: who may act, which
//! executor they run on, the skills they carry and what their runs did,
//! rendered from a wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`agents.props`: connected, dark,
//! the signing account, and the run another tab opened for the reader). The
//! register, the run tracker and one run's journal are read here through
//! the kernel's `rpc.query` / `rpc.view`, re-read on every `rpc.live` hit
//! for the `runs` and `identity` planes, and a pause or a save leaves as
//! `op.submit` — the runs message the kernel signs with the seated key. The
//! endpoint, the key and the password never cross: a guest that sees no key
//! cannot leak one.
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
pub struct AgentsView {
    pub(crate) active_palette: AppTheme,
    pub(crate) rows: Vec<crate::host::AgentRow>,
    pub(crate) runs: Vec<crate::host::RunRow>,
    pub(crate) journal: crate::host::RunJournal,
    pub(crate) live: crate::host::LiveRun,
    pub(crate) panel: String,
    pub(crate) open_run: String,
    pub(crate) journal_width: f64,
    pub(crate) editor_width: f64,
    pub(crate) viewport_width: f64,
    pub(crate) expanded_receipt: String,
    pub(crate) open_row: crate::host::RunRow,
    pub(crate) opened: i64,
    pub(crate) capabilities: Vec<String>,
    pub(crate) account: String,
    pub(crate) committed: i64,
    pub(crate) seeded: i64,
    pub(crate) connected: bool,
    pub(crate) connection_serial: i64,
    pub(crate) answered: bool,
    pub(crate) host_error: String,
    pub(crate) selected: String,
    pub(crate) creating: bool,
    pub(crate) can_edit: bool,
    pub(crate) selected_status: String,
    pub(crate) draft_id: String,
    pub(crate) draft_name: String,
    pub(crate) draft_capability: Option<String>,
    pub(crate) draft_skills: Vec<crate::host::AgentSkill>,
    pub(crate) skill_name: String,
    pub(crate) skill_prefix: String,
    pub(crate) skill_snapshot: String,
    pub(crate) skill_always: bool,
    pub(crate) sent: bool,
}
impl ::std::fmt::Debug for AgentsView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("AgentsView")
    }
}
#[derive(Clone)]
pub enum Message {
    JournalResized(f64, f64),
    EditorResized(f64, f64),
    ViewportChanged(f64, f64),
    ToggleReceipt(String),
    SessionArrived(crate::host::SessionItem),
    RegisterArrived(crate::host::RegisterItem),
    JournalArrived(crate::host::JournalItem),
    LiveArrived(crate::host::LiveRun),
    ActDone(crate::host::ActItem),
    OpenAgent(String),
    OpenNew,
    CloseEditor,
    ChoosePanel(String),
    OpenRunRow(String),
    CloseRun,
    OpenPlace(String),
    PickCapabilityOption(String),
    SetSkillAlways(bool),
    AddSkill,
    RemoveSkill(String),
    LoadSkill(String, bool),
    SetStatus(String, bool),
    SubmitSave,
    SubmitRegister,
    BindDraftId(String),
    BindDraftName(String),
    BindSkillName(String),
    BindSkillPrefix(String),
    BindSkillSnapshot(String),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl AgentsView {
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
impl AgentsView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            rows: Vec::new(),
            runs: Vec::new(),
            journal: crate::host::empty_journal(),
            live: crate::host::empty_live(),
            panel: "registry".to_owned(),
            open_run: "".to_owned(),
            journal_width: 400.0,
            editor_width: 400.0,
            viewport_width: 1280.0,
            expanded_receipt: "".to_owned(),
            open_row: crate::host::empty_run(),
            opened: 0,
            capabilities: Vec::new(),
            account: "".to_owned(),
            committed: 0,
            seeded: 0,
            connected: false,
            connection_serial: 0,
            answered: false,
            host_error: "".to_owned(),
            selected: "".to_owned(),
            creating: false,
            can_edit: false,
            selected_status: "".to_owned(),
            draft_id: "".to_owned(),
            draft_name: "".to_owned(),
            draft_capability: None,
            draft_skills: Vec::new(),
            skill_name: "".to_owned(),
            skill_prefix: "".to_owned(),
            skill_snapshot: "".to_owned(),
            skill_always: false,
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
        rows: Vec<crate::host::AgentRow>,
        runs: Vec<crate::host::RunRow>,
        journal: crate::host::RunJournal,
        live: crate::host::LiveRun,
        panel: String,
        open_run: String,
        journal_width: f64,
        editor_width: f64,
        viewport_width: f64,
        expanded_receipt: String,
        open_row: crate::host::RunRow,
        opened: i64,
        capabilities: Vec<String>,
        account: String,
        committed: i64,
        seeded: i64,
        connected: bool,
        connection_serial: i64,
        answered: bool,
        host_error: String,
        selected: String,
        creating: bool,
        can_edit: bool,
        selected_status: String,
        draft_id: String,
        draft_name: String,
        draft_capability: Option<String>,
        draft_skills: Vec<crate::host::AgentSkill>,
        skill_name: String,
        skill_prefix: String,
        skill_snapshot: String,
        skill_always: bool,
        sent: bool,
    ) -> Self {
        Self {
            active_palette: active_palette,
            rows: rows,
            runs: runs,
            journal: journal,
            live: live,
            panel: panel,
            open_run: open_run,
            journal_width: journal_width,
            editor_width: editor_width,
            viewport_width: viewport_width,
            expanded_receipt: expanded_receipt,
            open_row: open_row,
            opened: opened,
            capabilities: capabilities,
            account: account,
            committed: committed,
            seeded: seeded,
            connected: connected,
            connection_serial: connection_serial,
            answered: answered,
            host_error: host_error,
            selected: selected,
            creating: creating,
            can_edit: can_edit,
            selected_status: selected_status,
            draft_id: draft_id,
            draft_name: draft_name,
            draft_capability: draft_capability,
            draft_skills: draft_skills,
            skill_name: skill_name,
            skill_prefix: skill_prefix,
            skill_snapshot: skill_snapshot,
            skill_always: skill_always,
            sent: sent,
        }
    }
    pub(crate) const SNAPSHOT_SCHEMA: &'static str = "ea29d77b6e03069ca0c580bd8bf52d687528c49195c03419d1d233b539e53355";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("AgentsView"),
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
                    name : String::from("AgentRow"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).name))), (String::from("initials"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).initials))), (String::from("capability"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).capability))), (String::from("status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).status))), (String::from("owner_handle"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).owner_handle))), (String::from("controller"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).controller))), (String::from("live"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).live))),
                    (String::from("skills"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).skills)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AgentSkill"), fields :
                    ::std::vec![(String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).name))), (String::from("source_prefix"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).source_prefix))), (String::from("source_snapshot"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).source_snapshot))), (String::from("always"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .always)))] }).collect()))] }).collect())), (String::from("runs"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.runs).iter()
                    .map(| item | ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("RunRow"), fields :
                    ::std::vec![(String::from("run_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).run_id))), (String::from("dispatch_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).dispatch_id))), (String::from("agent_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).agent_id))), (String::from("agent_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).agent_name))), (String::from("origin"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).origin))), (String::from("state"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).state))), (String::from("dispatched"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).dispatched))), (String::from("settled"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).settled))), (String::from("attempt"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .attempt))), (String::from("holder"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).holder))), (String::from("actions"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .actions))), (String::from("degraded"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .degraded))), (String::from("reason"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).reason))), (String::from("output_ref"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).output_ref))), (String::from("pr_number"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .pr_number)))] }).collect())), (String::from("journal"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("RunJournal"), fields :
                    ::std::vec![(String::from("dispatch_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.journal).dispatch_id))), (String::from("entries"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (& self.journal)
                    .entries).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("JournalEntry"), fields :
                    ::std::vec![(String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).height))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("summary"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).summary))), (String::from("status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).status))), (String::from("targets"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).targets)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("RunLink"), fields :
                    ::std::vec![(String::from("relation"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).relation))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("label"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).label))), (String::from("url"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).url)))] }).collect()))] }).collect())),
                    (String::from("links"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (& self.journal)
                    .links).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("RunLink"), fields :
                    ::std::vec![(String::from("relation"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).relation))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("label"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).label))), (String::from("url"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).url)))] }).collect()))] }), (String::from("live"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("LiveRun"), fields :
                    ::std::vec![(String::from("present"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (& self.live)
                    .present))), (String::from("status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.live).status))), (String::from("activity"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (& self.live)
                    .activity).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("LiveActivity"), fields :
                    ::std::vec![(String::from("label"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).label))), (String::from("done"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).done)))]
                    }).collect())), (String::from("answer_preview"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.live).answer_preview)))] }), (String::from("panel"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.panel))), (String::from("open_run"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.open_run))), (String::from("journal_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .journal_width))), (String::from("editor_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .editor_width))), (String::from("viewport_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .viewport_width))), (String::from("expanded_receipt"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.expanded_receipt))), (String::from("open_row"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("RunRow"), fields : ::std::vec![(String::from("run_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).run_id))), (String::from("dispatch_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).dispatch_id))), (String::from("agent_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).agent_id))), (String::from("agent_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).agent_name))), (String::from("origin"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).origin))), (String::from("state"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).state))), (String::from("dispatched"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).dispatched))), (String::from("settled"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).settled))), (String::from("attempt"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .open_row).attempt))), (String::from("holder"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).holder))), (String::from("actions"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .open_row).actions))), (String::from("degraded"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (& self
                    .open_row).degraded))), (String::from("reason"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).reason))), (String::from("output_ref"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.open_row).output_ref))), (String::from("pr_number"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .open_row).pr_number)))] }), (String::from("opened"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self.opened))),
                    (String::from("capabilities"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .capabilities).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(item)))
                    .collect())), (String::from("account"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account))), (String::from("committed"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .committed))), (String::from("seeded"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self.seeded))),
                    (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected))), (String::from("connection_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .connection_serial))), (String::from("answered"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .answered))), (String::from("host_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.host_error))), (String::from("selected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.selected))), (String::from("creating"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .creating))), (String::from("can_edit"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .can_edit))), (String::from("selected_status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.selected_status))), (String::from("draft_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.draft_id))), (String::from("draft_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.draft_name))), (String::from("draft_capability"),
                    ::ducktape_view_guest::wire::SnapshotValue::Option((& self
                    .draft_capability).as_ref().map(| item |
                    Box::new(::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(item)))))),
                    (String::from("draft_skills"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .draft_skills).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AgentSkill"), fields :
                    ::std::vec![(String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).name))), (String::from("source_prefix"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).source_prefix))), (String::from("source_snapshot"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).source_snapshot))), (String::from("always"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .always)))] }).collect())), (String::from("skill_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.skill_name))), (String::from("skill_prefix"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.skill_prefix))), (String::from("skill_snapshot"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.skill_snapshot))), (String::from("skill_always"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .skill_always))), (String::from("sent"),
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
            if name != "AgentsView" || fields.len() != 34 {
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
            let rows: Vec<crate::host::AgentRow> = (match value {
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
                            if name != "AgentRow" || fields.len() != 9 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "name" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "initials" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "capability" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "status" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "owner_handle" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "controller" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "live" {
                                return None;
                            }
                            let (name, field_8) = fields.next()?;
                            if name != "skills" {
                                return None;
                            }
                            Some(crate::host::AgentRow {
                                id: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                name: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                initials: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                capability: (match field_3 {
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
                                owner_handle: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                controller: (match field_6 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                live: (match field_7 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                skills: (match field_8 {
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
                                                if name != "AgentSkill" || fields.len() != 4 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "name" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "source_prefix" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "source_snapshot" {
                                                    return None;
                                                }
                                                let (name, field_3) = fields.next()?;
                                                if name != "always" {
                                                    return None;
                                                }
                                                Some(crate::host::AgentSkill {
                                                    name: (match field_0 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    source_prefix: (match field_1 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    source_snapshot: (match field_2 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    always: (match field_3 {
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
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "runs" {
                return None;
            }
            let runs: Vec<crate::host::RunRow> = (match value {
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
                            if name != "RunRow" || fields.len() != 15 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "run_id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "dispatch_id" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "agent_id" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "agent_name" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "origin" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "state" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "dispatched" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "settled" {
                                return None;
                            }
                            let (name, field_8) = fields.next()?;
                            if name != "attempt" {
                                return None;
                            }
                            let (name, field_9) = fields.next()?;
                            if name != "holder" {
                                return None;
                            }
                            let (name, field_10) = fields.next()?;
                            if name != "actions" {
                                return None;
                            }
                            let (name, field_11) = fields.next()?;
                            if name != "degraded" {
                                return None;
                            }
                            let (name, field_12) = fields.next()?;
                            if name != "reason" {
                                return None;
                            }
                            let (name, field_13) = fields.next()?;
                            if name != "output_ref" {
                                return None;
                            }
                            let (name, field_14) = fields.next()?;
                            if name != "pr_number" {
                                return None;
                            }
                            Some(crate::host::RunRow {
                                run_id: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                dispatch_id: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                agent_id: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                agent_name: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                origin: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                state: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                dispatched: (match field_6 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                settled: (match field_7 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                attempt: (match field_8 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                holder: (match field_9 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                actions: (match field_10 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                degraded: (match field_11 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                reason: (match field_12 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                output_ref: (match field_13 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                pr_number: (match field_14 {
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
            if name != "journal" {
                return None;
            }
            let journal: crate::host::RunJournal = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "RunJournal" || fields.len() != 3 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "dispatch_id" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "entries" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "links" {
                    return None;
                }
                Some(crate::host::RunJournal {
                    dispatch_id: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    entries: (match field_1 {
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
                                    if name != "JournalEntry" || fields.len() != 5 {
                                        return None;
                                    }
                                    let mut fields = fields.into_iter();
                                    let (name, field_0) = fields.next()?;
                                    if name != "height" {
                                        return None;
                                    }
                                    let (name, field_1) = fields.next()?;
                                    if name != "kind" {
                                        return None;
                                    }
                                    let (name, field_2) = fields.next()?;
                                    if name != "summary" {
                                        return None;
                                    }
                                    let (name, field_3) = fields.next()?;
                                    if name != "status" {
                                        return None;
                                    }
                                    let (name, field_4) = fields.next()?;
                                    if name != "targets" {
                                        return None;
                                    }
                                    Some(crate::host::JournalEntry {
                                        height: (match field_0 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        kind: (match field_1 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        summary: (match field_2 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        status: (match field_3 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        targets: (match field_4 {
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
                                                        if name != "RunLink" || fields.len() != 4 {
                                                            return None;
                                                        }
                                                        let mut fields = fields.into_iter();
                                                        let (name, field_0) = fields.next()?;
                                                        if name != "relation" {
                                                            return None;
                                                        }
                                                        let (name, field_1) = fields.next()?;
                                                        if name != "kind" {
                                                            return None;
                                                        }
                                                        let (name, field_2) = fields.next()?;
                                                        if name != "label" {
                                                            return None;
                                                        }
                                                        let (name, field_3) = fields.next()?;
                                                        if name != "url" {
                                                            return None;
                                                        }
                                                        Some(crate::host::RunLink {
                                                            relation: (match field_0 {
                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                    Some(item)
                                                                }
                                                                _ => None,
                                                            })?,
                                                            kind: (match field_1 {
                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                    Some(item)
                                                                }
                                                                _ => None,
                                                            })?,
                                                            label: (match field_2 {
                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                    Some(item)
                                                                }
                                                                _ => None,
                                                            })?,
                                                            url: (match field_3 {
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
                                        })?,
                                    })
                                })())
                                .collect::<Option<Vec<_>>>()
                        }
                        _ => None,
                    })?,
                    links: (match field_2 {
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
                                    if name != "RunLink" || fields.len() != 4 {
                                        return None;
                                    }
                                    let mut fields = fields.into_iter();
                                    let (name, field_0) = fields.next()?;
                                    if name != "relation" {
                                        return None;
                                    }
                                    let (name, field_1) = fields.next()?;
                                    if name != "kind" {
                                        return None;
                                    }
                                    let (name, field_2) = fields.next()?;
                                    if name != "label" {
                                        return None;
                                    }
                                    let (name, field_3) = fields.next()?;
                                    if name != "url" {
                                        return None;
                                    }
                                    Some(crate::host::RunLink {
                                        relation: (match field_0 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        kind: (match field_1 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        label: (match field_2 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        url: (match field_3 {
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
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "live" {
                return None;
            }
            let live: crate::host::LiveRun = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "LiveRun" || fields.len() != 4 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "present" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "status" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "activity" {
                    return None;
                }
                let (name, field_3) = fields.next()?;
                if name != "answer_preview" {
                    return None;
                }
                Some(crate::host::LiveRun {
                    present: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    status: (match field_1 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    activity: (match field_2 {
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
                                    if name != "LiveActivity" || fields.len() != 2 {
                                        return None;
                                    }
                                    let mut fields = fields.into_iter();
                                    let (name, field_0) = fields.next()?;
                                    if name != "label" {
                                        return None;
                                    }
                                    let (name, field_1) = fields.next()?;
                                    if name != "done" {
                                        return None;
                                    }
                                    Some(crate::host::LiveActivity {
                                        label: (match field_0 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        done: (match field_1 {
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
                    })?,
                    answer_preview: (match field_3 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "panel" {
                return None;
            }
            let panel: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "open_run" {
                return None;
            }
            let open_run: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "journal_width" {
                return None;
            }
            let journal_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "editor_width" {
                return None;
            }
            let editor_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
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
            if name != "expanded_receipt" {
                return None;
            }
            let expanded_receipt: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "open_row" {
                return None;
            }
            let open_row: crate::host::RunRow = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "RunRow" || fields.len() != 15 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "run_id" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "dispatch_id" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "agent_id" {
                    return None;
                }
                let (name, field_3) = fields.next()?;
                if name != "agent_name" {
                    return None;
                }
                let (name, field_4) = fields.next()?;
                if name != "origin" {
                    return None;
                }
                let (name, field_5) = fields.next()?;
                if name != "state" {
                    return None;
                }
                let (name, field_6) = fields.next()?;
                if name != "dispatched" {
                    return None;
                }
                let (name, field_7) = fields.next()?;
                if name != "settled" {
                    return None;
                }
                let (name, field_8) = fields.next()?;
                if name != "attempt" {
                    return None;
                }
                let (name, field_9) = fields.next()?;
                if name != "holder" {
                    return None;
                }
                let (name, field_10) = fields.next()?;
                if name != "actions" {
                    return None;
                }
                let (name, field_11) = fields.next()?;
                if name != "degraded" {
                    return None;
                }
                let (name, field_12) = fields.next()?;
                if name != "reason" {
                    return None;
                }
                let (name, field_13) = fields.next()?;
                if name != "output_ref" {
                    return None;
                }
                let (name, field_14) = fields.next()?;
                if name != "pr_number" {
                    return None;
                }
                Some(crate::host::RunRow {
                    run_id: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    dispatch_id: (match field_1 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    agent_id: (match field_2 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    agent_name: (match field_3 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    origin: (match field_4 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    state: (match field_5 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    dispatched: (match field_6 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    settled: (match field_7 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    attempt: (match field_8 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    holder: (match field_9 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    actions: (match field_10 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    degraded: (match field_11 {
                        ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    reason: (match field_12 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    output_ref: (match field_13 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    pr_number: (match field_14 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "opened" {
                return None;
            }
            let opened: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "capabilities" {
                return None;
            }
            let capabilities: Vec<String> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| match item {
                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                Some(item)
                            }
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account" {
                return None;
            }
            let account: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "committed" {
                return None;
            }
            let committed: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "seeded" {
                return None;
            }
            let seeded: i64 = (match value {
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
            if name != "selected" {
                return None;
            }
            let selected: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "creating" {
                return None;
            }
            let creating: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "can_edit" {
                return None;
            }
            let can_edit: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "selected_status" {
                return None;
            }
            let selected_status: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "draft_id" {
                return None;
            }
            let draft_id: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "draft_name" {
                return None;
            }
            let draft_name: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "draft_capability" {
                return None;
            }
            let draft_capability: Option<String> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Option(None) => Some(None),
                ::ducktape_view_guest::wire::SnapshotValue::Option(Some(item)) => {
                    (match *item {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })
                        .map(Some)
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "draft_skills" {
                return None;
            }
            let draft_skills: Vec<crate::host::AgentSkill> = (match value {
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
                            if name != "AgentSkill" || fields.len() != 4 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "name" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "source_prefix" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "source_snapshot" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "always" {
                                return None;
                            }
                            Some(crate::host::AgentSkill {
                                name: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                source_prefix: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                source_snapshot: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                always: (match field_3 {
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
            if name != "skill_name" {
                return None;
            }
            let skill_name: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "skill_prefix" {
                return None;
            }
            let skill_prefix: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "skill_snapshot" {
                return None;
            }
            let skill_snapshot: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "skill_always" {
                return None;
            }
            let skill_always: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
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
                    rows,
                    runs,
                    journal,
                    live,
                    panel,
                    open_run,
                    journal_width,
                    editor_width,
                    viewport_width,
                    expanded_receipt,
                    open_row,
                    opened,
                    capabilities,
                    account,
                    committed,
                    seeded,
                    connected,
                    connection_serial,
                    answered,
                    host_error,
                    selected,
                    creating,
                    can_edit,
                    selected_status,
                    draft_id,
                    draft_name,
                    draft_capability,
                    draft_skills,
                    skill_name,
                    skill_prefix,
                    skill_snapshot,
                    skill_always,
                    sent,
                ),
            )
        })())
            .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl AgentsView {
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
            if (self.connected && (!(self.open_run).is_empty())) {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::run_journal(
                            self.open_run.to_owned(),
                            self.connection_serial,
                        )
                        .map(move |value| Message::JournalArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (!(self.open_run).is_empty())) {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::live_run(
                            self.open_run.to_owned(),
                            self.connection_serial,
                        )
                        .map(move |value| Message::LiveArrived(value)),
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
                let (app, _) = AgentsView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
impl AgentsView {
    pub(crate) fn render_run_chip_0(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: crate::host::RunLink,
    ) -> ::ducktape_view_guest::wire::Node {
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
            key: format!("{}/@container:1393", use_scope),
            width: Some(::ducktape_view_guest::wire::Length::Fill),
            height: None,
            padding: Some(::ducktape_view_guest::wire::Edges {
                top: (4.0) as f32,
                right: (9.0) as f32,
                bottom: (4.0) as f32,
                left: (9.0) as f32,
            }),
            align_x: None,
            align_y: None,
            background: (None).map(::ducktape_view_guest::wire::Background::Color),
            border: Some(::ducktape_view_guest::wire::Border {
                color: None,
                width: None,
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
                        key: format!("{}/@text:1401", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[71]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                        },
                        width: None,
                        align_x: None,
                        content: (crate::host::link_glyph(
                            ::std::convert::AsRef::as_ref(&(arg_0.kind)),
                        ))
                            .to_string(),
                    });
                if (arg_0.relation == "from") {
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
                            key: format!("{}/@text:1407", use_scope),
                            size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[72]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                            },
                            width: None,
                            align_x: None,
                            content: ("from".to_owned()).to_string(),
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
                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                            }),
                        },
                        key: format!("{}/@text:1412", use_scope),
                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                        color: Some(palette.colors[4]),
                        font: ::ducktape_view_guest::wire::Font {
                            monospace: false,
                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                        },
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        align_x: None,
                        content: (arg_0.label.to_owned()).to_string(),
                    });
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: format!("{}/@layout:1400", use_scope),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Row,
                    spacing: Some((6.0) as f32),
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
impl AgentsView {
    #[allow(clippy::assign_op_pattern)]
    pub(crate) fn update(
        &mut self,
        message: Message,
    ) -> ::ducktape_view_guest::Task<Message> {
        match message {
            Message::JournalResized(dx, _dy) => self.on_journal_resized(dx, _dy),
            Message::EditorResized(dx, _dy) => self.on_editor_resized(dx, _dy),
            Message::ViewportChanged(width, _height) => {
                self.on_viewport_changed(width, _height)
            }
            Message::ToggleReceipt(value) => self.on_toggle_receipt(value),
            Message::SessionArrived(item) => self.on_session_arrived(item),
            Message::RegisterArrived(item) => self.on_register_arrived(item),
            Message::JournalArrived(item) => self.on_journal_arrived(item),
            Message::LiveArrived(item) => self.on_live_arrived(item),
            Message::ActDone(item) => self.on_act_done(item),
            Message::OpenAgent(id) => self.on_open_agent(id),
            Message::OpenNew => self.on_open_new(),
            Message::CloseEditor => self.on_close_editor(),
            Message::ChoosePanel(next) => self.on_choose_panel(next),
            Message::OpenRunRow(run_id) => self.on_open_run_row(run_id),
            Message::CloseRun => self.on_close_run(),
            Message::OpenPlace(url) => self.on_open_place(url),
            Message::PickCapabilityOption(value) => self.on_pick_capability_option(value),
            Message::SetSkillAlways(on) => self.on_set_skill_always(on),
            Message::AddSkill => self.on_add_skill(),
            Message::RemoveSkill(name) => self.on_remove_skill(name),
            Message::LoadSkill(name, always) => self.on_load_skill(name, always),
            Message::SetStatus(agent_id, paused) => self.on_set_status(agent_id, paused),
            Message::SubmitSave => self.on_submit_save(),
            Message::SubmitRegister => self.on_submit_register(),
            Message::BindDraftId(value) => self.on_bind_draft_id(value),
            Message::BindDraftName(value) => self.on_bind_draft_name(value),
            Message::BindSkillName(value) => self.on_bind_skill_name(value),
            Message::BindSkillPrefix(value) => self.on_bind_skill_prefix(value),
            Message::BindSkillSnapshot(value) => self.on_bind_skill_snapshot(value),
        }
    }
    fn on_journal_resized(
        &mut self,
        dx: f64,
        _dy: f64,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = crate::host::journal_width_after_delta(
                    self.journal_width,
                    (-dx),
                    self.viewport_width,
                );
                self.journal_width = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_editor_resized(
        &mut self,
        dx: f64,
        _dy: f64,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = crate::host::editor_width_after_delta(
                    self.editor_width,
                    (-dx),
                    self.viewport_width,
                );
                self.editor_width = next;
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
                let next = width;
                self.viewport_width = next;
            }
            {
                let next = crate::host::journal_width_after_delta(
                    self.journal_width,
                    0.0,
                    width,
                );
                self.journal_width = next;
            }
            {
                let next = crate::host::editor_width_after_delta(
                    self.editor_width,
                    0.0,
                    width,
                );
                self.editor_width = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_toggle_receipt(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = crate::host::pick_str(
                    (self.expanded_receipt != value),
                    ::std::convert::AsRef::as_ref(&(value)),
                    ::std::convert::AsRef::as_ref(&("")),
                );
                self.expanded_receipt = next;
            }
            ::ducktape_view_guest::Task::none()
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
                let next = next.account.to_owned();
                self.account = next;
            }
            {
                let next = next.open_run.to_owned();
                self.open_run = next;
            }
            {
                let next = crate::host::run_at(
                    ::std::convert::AsRef::as_ref(&(self.runs)),
                    ::std::convert::AsRef::as_ref(&(self.open_run)),
                );
                self.open_row = next;
            }
            let door_pressed = ((next.opened != self.opened)
                && (!(next.open_run).is_empty()));
            {
                let next = next.opened;
                self.opened = next;
            }
            {
                let next = crate::host::pick_str(
                    door_pressed,
                    ::std::convert::AsRef::as_ref(&("runs")),
                    ::std::convert::AsRef::as_ref(&(self.panel)),
                );
                self.panel = next;
            }
            let row = crate::host::row_named(
                ::std::convert::AsRef::as_ref(&(self.rows)),
                ::std::convert::AsRef::as_ref(&(self.selected)),
            );
            {
                let next = crate::host::editable(
                    self.connected,
                    ::std::convert::AsRef::as_ref(&(self.account)),
                    ::std::convert::AsRef::as_ref(&(row.controller)),
                );
                self.can_edit = next;
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
    fn on_register_arrived(
        &mut self,
        item: crate::host::RegisterItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = item.error.to_owned();
                self.host_error = next;
            }
            {
                let next = true;
                self.answered = next;
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                let next = item.rows.clone();
                self.rows = next;
            }
            {
                let next = item.runs.clone();
                self.runs = next;
            }
            {
                let next = item.capabilities.clone();
                self.capabilities = next;
            }
            {
                let next = crate::host::run_at(
                    ::std::convert::AsRef::as_ref(&(self.runs)),
                    ::std::convert::AsRef::as_ref(&(self.open_run)),
                );
                self.open_row = next;
            }
            {
                let next = (crate::host::badge(
                    crate::host::working_agents(
                        ::std::convert::AsRef::as_ref(&(self.rows)),
                    ),
                ));
                self.sent = next;
            }
            let consumed = crate::host::drafts_consumed(
                self.committed,
                self.seeded,
                self.creating,
                ::std::convert::AsRef::as_ref(&(self.rows)),
                ::std::convert::AsRef::as_ref(&(self.draft_id)),
            );
            {
                let next = self.committed;
                self.seeded = next;
            }
            {
                let next = crate::host::pick_str(
                    (consumed && self.creating),
                    ::std::convert::AsRef::as_ref(&(self.draft_id)),
                    ::std::convert::AsRef::as_ref(&(self.selected)),
                );
                self.selected = next;
            }
            {
                let next = (self.creating && (!consumed));
                self.creating = next;
            }
            let row = crate::host::row_named(
                ::std::convert::AsRef::as_ref(&(self.rows)),
                ::std::convert::AsRef::as_ref(&(self.selected)),
            );
            {
                let next = crate::host::editable(
                    self.connected,
                    ::std::convert::AsRef::as_ref(&(self.account)),
                    ::std::convert::AsRef::as_ref(&(row.controller)),
                );
                self.can_edit = next;
            }
            {
                let next = row.status.to_owned();
                self.selected_status = next;
            }
            {
                let next = crate::host::pick_str(
                    consumed,
                    ::std::convert::AsRef::as_ref(&(row.name)),
                    ::std::convert::AsRef::as_ref(&(self.draft_name)),
                );
                self.draft_name = next;
            }
            {
                let next = crate::host::pick_capability(
                    consumed,
                    ::std::convert::AsRef::as_ref(&(row.capability)),
                    ::std::borrow::Borrow::borrow(&(self.draft_capability)),
                );
                self.draft_capability = next;
            }
            {
                let next = crate::host::pick_skills(
                    consumed,
                    ::std::convert::AsRef::as_ref(&(row.skills)),
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                );
                self.draft_skills = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_journal_arrived(
        &mut self,
        item: crate::host::JournalItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = item.error.to_owned();
                self.host_error = next;
            }
            if ((!(item.error).is_empty())
                || (item.journal.dispatch_id != self.open_run))
            {
                return ::ducktape_view_guest::Task::none();
            }
            {
                let next = item.journal.clone();
                self.journal = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_live_arrived(
        &mut self,
        item: crate::host::LiveRun,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = item.clone();
                self.live = next;
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
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                let next = (self.committed + 1);
                self.committed = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_agent(&mut self, id: String) -> ::ducktape_view_guest::Task<Message> {
        {
            let row = crate::host::row_named(
                ::std::convert::AsRef::as_ref(&(self.rows)),
                ::std::convert::AsRef::as_ref(&(id)),
            );
            {
                let next = id.to_owned();
                self.selected = next;
            }
            {
                let next = false;
                self.creating = next;
            }
            {
                let next = crate::host::editable(
                    self.connected,
                    ::std::convert::AsRef::as_ref(&(self.account)),
                    ::std::convert::AsRef::as_ref(&(row.controller)),
                );
                self.can_edit = next;
            }
            {
                let next = row.status.to_owned();
                self.selected_status = next;
            }
            {
                let next = row.id.to_owned();
                self.draft_id = next;
            }
            {
                let next = row.name.to_owned();
                self.draft_name = next;
            }
            {
                let next = crate::host::some_str(
                    ::std::convert::AsRef::as_ref(&(row.capability)),
                );
                self.draft_capability = next;
            }
            {
                let next = row.skills.clone();
                self.draft_skills = next;
            }
            {
                let next = "".to_owned();
                self.skill_name = next;
            }
            {
                let next = "".to_owned();
                self.skill_prefix = next;
            }
            {
                let next = "".to_owned();
                self.skill_snapshot = next;
            }
            {
                let next = false;
                self.skill_always = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_new(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = "registry".to_owned();
                self.panel = next;
            }
            {
                let next = "".to_owned();
                self.selected = next;
            }
            {
                let next = true;
                self.creating = next;
            }
            {
                let next = (self.connected && (!(self.account).is_empty()));
                self.can_edit = next;
            }
            {
                let next = "".to_owned();
                self.draft_id = next;
            }
            {
                let next = "".to_owned();
                self.draft_name = next;
            }
            {
                let next = None;
                self.draft_capability = next;
            }
            {
                let next = Vec::new();
                self.draft_skills = next;
            }
            {
                let next = "".to_owned();
                self.skill_name = next;
            }
            {
                let next = "".to_owned();
                self.skill_prefix = next;
            }
            {
                let next = "".to_owned();
                self.skill_snapshot = next;
            }
            {
                let next = false;
                self.skill_always = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_close_editor(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = "".to_owned();
                self.selected = next;
            }
            {
                let next = false;
                self.creating = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_choose_panel(&mut self, next: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = next.to_owned();
                self.panel = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_run_row(
        &mut self,
        run_id: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = "".to_owned();
                self.expanded_receipt = next;
            }
            {
                let next = crate::host::run_named(
                    ::std::convert::AsRef::as_ref(&(self.runs)),
                    ::std::convert::AsRef::as_ref(&(run_id)),
                );
                self.open_row = next;
            }
            {
                let next = self.open_row.dispatch_id.to_owned();
                self.open_run = next;
            }
            {
                let next = crate::host::open_run(
                    ::std::convert::AsRef::as_ref(&(self.open_row.dispatch_id)),
                );
                self.sent = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_close_run(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = "".to_owned();
                self.expanded_receipt = next;
            }
            {
                let next = "".to_owned();
                self.open_run = next;
            }
            {
                let next = crate::host::empty_run();
                self.open_row = next;
            }
            {
                let next = crate::host::empty_journal();
                self.journal = next;
            }
            {
                let next = crate::host::empty_live();
                self.live = next;
            }
            {
                let next = crate::host::open_run(::std::convert::AsRef::as_ref(&("")));
                self.sent = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_place(&mut self, url: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = crate::host::open_link(::std::convert::AsRef::as_ref(&(url)));
                self.sent = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_pick_capability_option(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = Some(value.to_owned());
                self.draft_capability = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_set_skill_always(&mut self, on: bool) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = on;
                self.skill_always = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_add_skill(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = crate::host::with_skill(
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                    ::std::convert::AsRef::as_ref(&(self.skill_name)),
                    ::std::convert::AsRef::as_ref(
                        &(crate::host::pick_str(
                            ((self.skill_prefix).trim().to_owned()).is_empty(),
                            ::std::convert::AsRef::as_ref(
                                &(crate::host::library_prefix(
                                    ::std::convert::AsRef::as_ref(&(self.skill_name)),
                                )),
                            ),
                            ::std::convert::AsRef::as_ref(&(self.skill_prefix)),
                        )),
                    ),
                    ::std::convert::AsRef::as_ref(&(self.skill_snapshot)),
                    self.skill_always,
                );
                self.draft_skills = next;
            }
            {
                let next = "".to_owned();
                self.skill_name = next;
            }
            {
                let next = "".to_owned();
                self.skill_prefix = next;
            }
            {
                let next = "".to_owned();
                self.skill_snapshot = next;
            }
            {
                let next = false;
                self.skill_always = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_remove_skill(&mut self, name: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = crate::host::without_skill(
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                    ::std::convert::AsRef::as_ref(&(name)),
                );
                self.draft_skills = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_load_skill(
        &mut self,
        name: String,
        always: bool,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = crate::host::skill_loaded(
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                    ::std::convert::AsRef::as_ref(&(name)),
                    always,
                );
                self.draft_skills = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_set_status(
        &mut self,
        agent_id: String,
        paused: bool,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = (crate::host::status(
                    ::std::convert::AsRef::as_ref(&(agent_id)),
                    paused,
                ));
                self.sent = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_submit_save(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = (crate::host::save(
                    ::std::convert::AsRef::as_ref(&(self.selected)),
                    ::std::convert::AsRef::as_ref(&(self.draft_name)),
                    ::std::convert::AsRef::as_ref(
                        &(crate::host::or_empty(
                            ::std::borrow::Borrow::borrow(&(self.draft_capability)),
                        )),
                    ),
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                ));
                self.sent = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_submit_register(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = crate::host::register_agent(
                    ::std::convert::AsRef::as_ref(&(self.draft_id)),
                    ::std::convert::AsRef::as_ref(&(self.draft_name)),
                    ::std::convert::AsRef::as_ref(
                        &(crate::host::or_empty(
                            ::std::borrow::Borrow::borrow(&(self.draft_capability)),
                        )),
                    ),
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                );
                self.sent = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_draft_id(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = value;
                self.draft_id = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_draft_name(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = value;
                self.draft_name = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_skill_name(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = value;
                self.skill_name = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_skill_prefix(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = value;
                self.skill_prefix = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_skill_snapshot(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                let next = value;
                self.skill_snapshot = next;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
}
impl AgentsView {
    pub(crate) fn view(&self) -> ::ducktape_view_guest::wire::Node {
        let palette = self.palette();
        {
            let node_scope = format!("{}/root", "AgentsView");
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
                            key: format!("{}/@sensor:337", node_scope),
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
                            key: format!("{}/@container:339", node_scope),
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
                                            content: ("Agents".to_owned()).to_string(),
                                        }
                                    });
                                if (self.panel == "registry") {
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
                                                content: (crate::host::agents_summary(
                                                    self.connected,
                                                    ::std::convert::AsRef::as_ref(&(self.rows)),
                                                ))
                                                    .to_string(),
                                            }
                                        });
                                }
                                if (self.panel == "runs") {
                                    children
                                        .push({
                                            let node_scope = format!("{}/runs-meta", node_scope);
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
                                                content: (crate::host::runs_summary(
                                                    ::std::convert::AsRef::as_ref(&(self.runs)),
                                                ))
                                                    .to_string(),
                                            }
                                        });
                                }
                                children
                                    .push(::ducktape_view_guest::wire::Node::Space {
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                    });
                                if self.connected {
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:377", node_scope),
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
                                                    key: format!("{}/@text:384", node_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: None,
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("Registry".to_owned()).to_string(),
                                                }),
                                            ),
                                            label: Some(String::from("Registry".to_owned())),
                                            on_press: if ((self.panel == "registry")) {
                                                None
                                            } else {
                                                Some(
                                                        ::ducktape_view_guest::slots::message(
                                                            Message::ChoosePanel("registry".to_owned()),
                                                        ),
                                                    )
                                            },
                                            width: None,
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                                            ),
                                            padding: Some(
                                                ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
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
                                }
                                if self.connected {
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:386", node_scope),
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
                                                    key: format!("{}/@text:393", node_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: None,
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("Runs".to_owned()).to_string(),
                                                }),
                                            ),
                                            label: Some(String::from("Runs".to_owned())),
                                            on_press: if ((self.panel == "runs")) {
                                                None
                                            } else {
                                                Some(
                                                        ::ducktape_view_guest::slots::message(
                                                            Message::ChoosePanel("runs".to_owned()),
                                                        ),
                                                    )
                                            },
                                            width: None,
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                                            ),
                                            padding: Some(
                                                ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
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
                                }
                                if (self.connected && (!(self.account).is_empty())) {
                                    children
                                        .push(::ducktape_view_guest::wire::Node::Button {
                                            checked: None,
                                            expanded: None,
                                            description: None,
                                            key: format!("{}/@button:397", node_scope),
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
                                                    key: format!("{}/@text:403", node_scope),
                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                    color: None,
                                                    font: ::ducktape_view_guest::wire::Font {
                                                        monospace: false,
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                    },
                                                    width: None,
                                                    align_x: None,
                                                    content: ("New agent".to_owned()).to_string(),
                                                }),
                                            ),
                                            label: Some(String::from("New agent".to_owned())),
                                            on_press: Some(
                                                ::ducktape_view_guest::slots::message(Message::OpenNew),
                                            ),
                                            width: None,
                                            height: Some(
                                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                                            ),
                                            padding: Some(
                                                ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
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
                                    key: format!("{}/@layout:344", node_scope),
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
                            key: format!("{}/@container:404", node_scope),
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
                    if (self.panel == "registry") {
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
                                key: format!("{}/@container:411", node_scope),
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                padding: Some(::ducktape_view_guest::wire::Edges {
                                    top: (12.0) as f32,
                                    right: (22.0) as f32,
                                    bottom: (10.0) as f32,
                                    left: (22.0) as f32,
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
                                    key: format!("{}/@text:417", node_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[70]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    align_x: None,
                                    content: (crate::host::pane_note(
                                        ::std::convert::AsRef::as_ref(&(self.panel)),
                                    ))
                                        .to_string(),
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
                                key: format!("{}/@container:422", node_scope),
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
                    }
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
                                                key: format!("{}/@container:445", node_scope),
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
                                                    key: format!("{}/@text:455", node_scope),
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
                                                key: format!("{}/@text:456", node_scope),
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
                                                key: format!("{}/@text:461", node_scope),
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
                                            key: format!("{}/@layout:440", node_scope),
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
                    if (self.connected && (self.panel == "runs")) {
                        children
                            .push({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                if (self.runs).is_empty() {
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
                                            key: format!("{}/@container:468", node_scope),
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
                                                let node_scope = format!("{}/no-runs", node_scope);
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
                                                        key: format!("{}/@text:481", node_scope),
                                                        size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[71]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: None,
                                                        align_x: None,
                                                        content: ("No runs yet — every dispatch of an agent lands here with its journal."
                                                            .to_owned())
                                                            .to_string(),
                                                    }),
                                                }
                                            }),
                                        });
                                }
                                if (!(self.runs).is_empty()) {
                                    children
                                        .push({
                                            let node_scope = format!("{}/runs-body", node_scope);
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
                                                    for (index, run) in self.runs.iter().enumerate() {
                                                        let for_scope = format!(
                                                            "{}/@for:1018({})", node_scope, index
                                                        );
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: None,
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:501", for_scope),
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
                                                                                key: format!("{}/@container:506", for_scope),
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
                                                                                                            key: format!("{}/@text:524", for_scope),
                                                                                                            size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[4]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: false,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (run.agent_name.to_owned()).to_string(),
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
                                                                                                            key: format!("{}/@text:529", for_scope),
                                                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[71]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                            align_x: None,
                                                                                                            content: (run.origin.to_owned()).to_string(),
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:519", for_scope),
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
                                                                                                            key: format!("{}/@text:540", for_scope),
                                                                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[72]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (run.dispatched.to_owned()).to_string(),
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
                                                                                                            key: format!("{}/@text:545", for_scope),
                                                                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[72]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
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
                                                                                                            key: format!("{}/@text:550", for_scope),
                                                                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[71]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (run.actions).to_string(),
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
                                                                                                            key: format!("{}/@text:556", for_scope),
                                                                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[71]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: ("actions".to_owned()).to_string(),
                                                                                                        });
                                                                                                    if (run.pr_number > 0) {
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
                                                                                                                key: format!("{}/@text:563", for_scope),
                                                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                color: Some(palette.colors[72]),
                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                    monospace: true,
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                },
                                                                                                                width: None,
                                                                                                                align_x: None,
                                                                                                                content: ("· PR #".to_owned()).to_string(),
                                                                                                            });
                                                                                                    }
                                                                                                    if (run.pr_number > 0) {
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
                                                                                                                key: format!("{}/@text:569", for_scope),
                                                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                color: Some(palette.colors[72]),
                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                    monospace: true,
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                                },
                                                                                                                width: None,
                                                                                                                align_x: None,
                                                                                                                content: (run.pr_number).to_string(),
                                                                                                            });
                                                                                                    }
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:535", for_scope),
                                                                                                        wrap: None,
                                                                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                                        spacing: Some((5.0) as f32),
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
                                                                                                key: format!("{}/@layout:518", for_scope),
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
                                                                                    if ((run.state == "dispatched") || (run.state == "running"))
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
                                                                                                key: format!("{}/@container:578", for_scope),
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
                                                                                                background: (Some(palette.colors[32]))
                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[33]),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
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
                                                                                                            key: format!("{}/@container:587", for_scope),
                                                                                                            width: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
                                                                                                            ),
                                                                                                            height: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
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
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
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
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:594", for_scope),
                                                                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[30]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (run.state.to_owned()).to_string(),
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:586", for_scope),
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
                                                                                            });
                                                                                    }
                                                                                    if (run.state == "accepted") {
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
                                                                                                key: format!("{}/@container:601", for_scope),
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
                                                                                                background: (Some(palette.colors[27]))
                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[28]),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
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
                                                                                                            key: format!("{}/@container:610", for_scope),
                                                                                                            width: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
                                                                                                            ),
                                                                                                            height: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
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
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
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
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:617", for_scope),
                                                                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[25]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (run.state.to_owned()).to_string(),
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:609", for_scope),
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
                                                                                            });
                                                                                    }
                                                                                    if ((run.state == "rejected") || (run.state == "failed")) {
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
                                                                                                key: format!("{}/@container:624", for_scope),
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
                                                                                                background: (Some(palette.colors[22]))
                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[23]),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
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
                                                                                                    key: format!("{}/@text:632", for_scope),
                                                                                                    size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[20]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: true,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: (run.state.to_owned()).to_string(),
                                                                                                }),
                                                                                            });
                                                                                    }
                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                        max_width: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@layout:513", for_scope),
                                                                                        wrap: None,
                                                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                        spacing: Some((13.0) as f32),
                                                                                        padding: None,
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        height: None,
                                                                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                        background: None,
                                                                                        border: None,
                                                                                        children: children,
                                                                                    }
                                                                                }),
                                                                            }),
                                                                        ),
                                                                        label: Some(String::from(run.run_id.to_owned())),
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(
                                                                                Message::OpenRunRow(run.run_id.to_owned()),
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
                                                                                background: Some(palette.colors[2]),
                                                                                text: None,
                                                                                border: None,
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
                                                                        key: format!("{}/@container:640", for_scope),
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
                                                                    key: format!("{}/@layout:500", for_scope),
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
                                                    }
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:491", node_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                        spacing: Some((11.0) as f32),
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (18.0) as f32,
                                                            right: (18.0) as f32,
                                                            bottom: (18.0) as f32,
                                                            left: (18.0) as f32,
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
                                if (!(self.open_run).is_empty()) {
                                    children
                                        .push({
                                            let node_scope = format!("{}/journal-resize", node_scope);
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
                                                            let route = move |delta: (f64, f64)| Message::JournalResized(
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
                                                    let node_scope = format!("{}/journal-divider", node_scope);
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
                                                            key: format!("{}/@container:656", node_scope),
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
                                            let node_scope = format!("{}/journal", node_scope);
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
                                                        (self.journal_width) as f32,
                                                    ),
                                                ),
                                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                padding: None,
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[3]))
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: Some(::ducktape_view_guest::wire::Border {
                                                    color: Some(palette.colors[39]),
                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                    radius: None,
                                                }),
                                                snap: None,
                                                content: Box::new(::ducktape_view_guest::wire::Node::Scroll {
                                                    on_scroll: None,
                                                    virtual_rows: false,
                                                    key: format!("{}/@layout:665", node_scope),
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
                                                                        key: format!("{}/@text:680", node_scope),
                                                                        size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[4]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: (self.open_row.agent_name.to_owned()).to_string(),
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
                                                                        key: format!("{}/@text:685", node_scope),
                                                                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[71]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: true,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: (self.open_row.state.to_owned()).to_string(),
                                                                    });
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Space {
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        height: None,
                                                                    });
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: None,
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:691", node_scope),
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
                                                                                key: format!("{}/@text:697", node_scope),
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
                                                                        label: Some(String::from("Close journal".to_owned())),
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(Message::CloseRun),
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
                                                                            active: ::ducktape_view_guest::wire::Face::default(),
                                                                            hovered: None,
                                                                            pressed: None,
                                                                            disabled: None,
                                                                        },
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:675", node_scope),
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
                                                                key: format!("{}/@text:698", node_scope),
                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[71]),
                                                                font: ::ducktape_view_guest::wire::Font {
                                                                    monospace: true,
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                },
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                align_x: None,
                                                                content: (self.open_row.origin.to_owned()).to_string(),
                                                            });
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                                checked: None,
                                                                expanded: Some((self.expanded_receipt == self.open_run)),
                                                                description: None,
                                                                key: format!("{}/@button:710", node_scope),
                                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                    Box::new({
                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                        if (self.expanded_receipt == self.open_run) {
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
                                                                                    key: format!("{}/@text:718", node_scope),
                                                                                    size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[71]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: ("▾".to_owned()).to_string(),
                                                                                });
                                                                        }
                                                                        if (self.expanded_receipt != self.open_run) {
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
                                                                                    key: format!("{}/@text:720", node_scope),
                                                                                    size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[71]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: ("▸".to_owned()).to_string(),
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
                                                                                key: format!("{}/@text:721", node_scope),
                                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[71]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("Details".to_owned()).to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:716", node_scope),
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
                                                                ),
                                                                label: Some(String::from("Run details".to_owned())),
                                                                on_press: Some(
                                                                    ::ducktape_view_guest::slots::message(
                                                                        Message::ToggleReceipt(self.open_run.to_owned()),
                                                                    ),
                                                                ),
                                                                width: None,
                                                                height: None,
                                                                padding: Some(
                                                                    ::ducktape_view_guest::wire::Edges::all((4.0) as f32),
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
                                                                    active: ::ducktape_view_guest::wire::Face::default(),
                                                                    hovered: None,
                                                                    pressed: None,
                                                                    disabled: None,
                                                                },
                                                            });
                                                        if (self.expanded_receipt == self.open_run) {
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
                                                                                        font: None,
                                                                                    },
                                                                                    key: format!("{}/@text:725", node_scope),
                                                                                    size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[73]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: true,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: Some(
                                                                                        ::ducktape_view_guest::wire::Length::Fixed((64.0) as f32),
                                                                                    ),
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
                                                                                        font: None,
                                                                                    },
                                                                                    key: format!("{}/@text:731", node_scope),
                                                                                    size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[71]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: true,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    align_x: None,
                                                                                    content: (self.open_run.to_owned()).to_string(),
                                                                                });
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:724", node_scope),
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
                                                                    if (!(self.open_row.output_ref).is_empty()) {
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
                                                                                        key: format!("{}/@text:739", node_scope),
                                                                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[73]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: true,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: Some(
                                                                                            ::ducktape_view_guest::wire::Length::Fixed((64.0) as f32),
                                                                                        ),
                                                                                        align_x: None,
                                                                                        content: ("output".to_owned()).to_string(),
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
                                                                                        key: format!("{}/@text:745", node_scope),
                                                                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[71]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: true,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        align_x: None,
                                                                                        content: (self.open_row.output_ref.to_owned()).to_string(),
                                                                                    });
                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                    max_width: None,
                                                                                    clip: false,
                                                                                    key: format!("{}/@layout:738", node_scope),
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
                                                                        key: format!("{}/@layout:723", node_scope),
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
                                                        }
                                                        if self.live.present {
                                                            children
                                                                .push({
                                                                    let node_scope = format!("{}/live", node_scope);
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
                                                                        background: (Some(palette.colors[32]))
                                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(palette.colors[33]),
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
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:767", node_scope),
                                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[4]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                    },
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    align_x: None,
                                                                                    content: (self.live.status.to_owned()).to_string(),
                                                                                });
                                                                            for (index, act) in self.live.activity.iter().enumerate() {
                                                                                let for_scope = format!(
                                                                                    "{}/@for:1292({})", node_scope, index
                                                                                );
                                                                                children
                                                                                    .push({
                                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                        if act.done {
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
                                                                                                    key: format!("{}/@text:776", for_scope),
                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[71]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: true,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: ("✓".to_owned()).to_string(),
                                                                                                });
                                                                                        }
                                                                                        if (!act.done) {
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
                                                                                                    key: format!("{}/@text:782", for_scope),
                                                                                                    size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: Some(palette.colors[71]),
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: true,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: ("…".to_owned()).to_string(),
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
                                                                                                key: format!("{}/@text:787", for_scope),
                                                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[71]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                                align_x: None,
                                                                                                content: (act.label.to_owned()).to_string(),
                                                                                            });
                                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                                            max_width: None,
                                                                                            clip: false,
                                                                                            key: format!("{}/@layout:774", for_scope),
                                                                                            wrap: None,
                                                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                            spacing: Some((5.0) as f32),
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
                                                                            if (!(self.live.answer_preview).is_empty()) {
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
                                                                                        key: format!("{}/@text:793", node_scope),
                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[71]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        align_x: None,
                                                                                        content: (self.live.answer_preview.to_owned()).to_string(),
                                                                                    });
                                                                            }
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:766", node_scope),
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
                                                                        }),
                                                                    }
                                                                });
                                                        }
                                                        if ((self.journal.dispatch_id == self.open_run)
                                                            && (!(self.journal.links).is_empty()))
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
                                                                                "Geist".into(),
                                                                            ),
                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:805", node_scope),
                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[4]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: ("Relevant".to_owned()).to_string(),
                                                                });
                                                            children
                                                                .push({
                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                    for (index, link) in self.journal.links.iter().enumerate() {
                                                                        let for_scope = format!(
                                                                            "{}/@for:1333({})", node_scope, index
                                                                        );
                                                                        if (!(link.url).is_empty()) {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:816", for_scope),
                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                                        Box::new(
                                                                                            self
                                                                                                .render_run_chip_0(
                                                                                                    palette,
                                                                                                    format!("{}/RunChip@1341", for_scope),
                                                                                                    link.clone(),
                                                                                                ),
                                                                                        ),
                                                                                    ),
                                                                                    label: Some(String::from(link.label.to_owned())),
                                                                                    on_press: Some(
                                                                                        ::ducktape_view_guest::slots::message(
                                                                                            Message::OpenPlace(link.url.to_owned()),
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
                                                                        }
                                                                        if (link.url).is_empty() {
                                                                            children
                                                                                .push(
                                                                                    self
                                                                                        .render_run_chip_0(
                                                                                            palette,
                                                                                            format!("{}/RunChip@1343", for_scope),
                                                                                            link.clone(),
                                                                                        ),
                                                                                );
                                                                        }
                                                                    }
                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:810", node_scope),
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
                                                                });
                                                        }
                                                        if (!(self.open_row.reason).is_empty()) {
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
                                                                    key: format!("{}/@container:826", node_scope),
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
                                                                    background: (Some(palette.colors[22]))
                                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some(palette.colors[23]),
                                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
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
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        },
                                                                        key: format!("{}/@text:835", node_scope),
                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[20]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        align_x: None,
                                                                        content: (self.open_row.reason.to_owned()).to_string(),
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
                                                                key: format!("{}/@text:840", node_scope),
                                                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[4]),
                                                                font: ::ducktape_view_guest::wire::Font {
                                                                    monospace: false,
                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                },
                                                                width: None,
                                                                align_x: None,
                                                                content: ("Journal".to_owned()).to_string(),
                                                            });
                                                        if (self.journal.dispatch_id != self.open_run) {
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
                                                                    key: format!("{}/@text:846", node_scope),
                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[70]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: None,
                                                                    align_x: None,
                                                                    content: ("Reading the journal…".to_owned()).to_string(),
                                                                });
                                                        }
                                                        if ((self.journal.dispatch_id == self.open_run)
                                                            && (self.journal.entries).is_empty())
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
                                                                                "Geist".into(),
                                                                            ),
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                        }),
                                                                    },
                                                                    key: format!("{}/@text:851", node_scope),
                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                    color: Some(palette.colors[70]),
                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                        monospace: false,
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    },
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    align_x: None,
                                                                    content: ("This run's journal has no entries yet — the fold may still be catching up to the chain."
                                                                        .to_owned())
                                                                        .to_string(),
                                                                });
                                                        }
                                                        if (self.journal.dispatch_id == self.open_run) {
                                                            for (index, entry) in self
                                                                .journal
                                                                .entries
                                                                .iter()
                                                                .enumerate()
                                                            {
                                                                let for_scope = format!(
                                                                    "{}/@for:1376({})", node_scope, index
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
                                                                                    font: None,
                                                                                },
                                                                                key: format!("{}/@text:859", for_scope),
                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[72]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: true,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (entry.height.to_owned()).to_string(),
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
                                                                                        key: format!("{}/@text:865", for_scope),
                                                                                        size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[4]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: true,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: (entry.kind.to_owned()).to_string(),
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
                                                                                        key: format!("{}/@text:871", for_scope),
                                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[71]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        align_x: None,
                                                                                        content: (entry.summary.to_owned()).to_string(),
                                                                                    });
                                                                                if (!(entry.status).is_empty()) {
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
                                                                                            key: format!("{}/@text:877", for_scope),
                                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[71]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: false,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: (entry.status.to_owned()).to_string(),
                                                                                        });
                                                                                }
                                                                                for (index, target) in entry.targets.iter().enumerate() {
                                                                                    let for_scope = format!(
                                                                                        "{}/@for:1397({})", for_scope, index
                                                                                    );
                                                                                    if (!(target.url).is_empty()) {
                                                                                        children
                                                                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                                                                checked: None,
                                                                                                expanded: None,
                                                                                                description: None,
                                                                                                key: format!("{}/@button:880", for_scope),
                                                                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                                                                    Box::new(
                                                                                                        self
                                                                                                            .render_run_chip_0(
                                                                                                                palette,
                                                                                                                format!("{}/RunChip@1405", for_scope),
                                                                                                                target.clone(),
                                                                                                            ),
                                                                                                    ),
                                                                                                ),
                                                                                                label: Some(String::from(target.label.to_owned())),
                                                                                                on_press: Some(
                                                                                                    ::ducktape_view_guest::slots::message(
                                                                                                        Message::OpenPlace(target.url.to_owned()),
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
                                                                                    }
                                                                                    if (target.url).is_empty() {
                                                                                        children
                                                                                            .push(
                                                                                                self
                                                                                                    .render_run_chip_0(
                                                                                                        palette,
                                                                                                        format!("{}/RunChip@1407", for_scope),
                                                                                                        target.clone(),
                                                                                                    ),
                                                                                            );
                                                                                    }
                                                                                }
                                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                                    max_width: None,
                                                                                    clip: false,
                                                                                    key: format!("{}/@layout:864", for_scope),
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
                                                                            key: format!("{}/@layout:858", for_scope),
                                                                            wrap: None,
                                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                            spacing: Some((8.0) as f32),
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
                                                            key: format!("{}/@layout:670", node_scope),
                                                            wrap: None,
                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                            spacing: Some((12.0) as f32),
                                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                                top: (18.0) as f32,
                                                                right: (18.0) as f32,
                                                                bottom: (18.0) as f32,
                                                                left: (18.0) as f32,
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
                                }
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:466", node_scope),
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
                    }
                    if ((((self.connected && (self.panel == "registry"))
                        && (self.rows).is_empty()) && self.answered) && (!self.creating))
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
                                key: format!("{}/@container:890", node_scope),
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
                                            key: format!("{}/@text:903", node_scope),
                                            size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                            color: Some(palette.colors[71]),
                                            font: ::ducktape_view_guest::wire::Font {
                                                monospace: false,
                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            },
                                            width: None,
                                            align_x: None,
                                            content: ("No model agents configured — models appear here with their capability and skills."
                                                .to_owned())
                                                .to_string(),
                                        }),
                                    }
                                }),
                            });
                    }
                    if ((self.connected && (self.panel == "registry"))
                        && ((!(self.rows).is_empty()) || self.creating))
                    {
                        children
                            .push({
                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                if (!(self.rows).is_empty()) {
                                    children
                                        .push({
                                            let node_scope = format!("{}/agents-body", node_scope);
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
                                                    for (index, agent) in self.rows.iter().enumerate() {
                                                        let for_scope = format!(
                                                            "{}/@for:1442({})", node_scope, index
                                                        );
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: None,
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:925", for_scope),
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
                                                                                key: format!("{}/@container:930", for_scope),
                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                height: None,
                                                                                padding: Some(::ducktape_view_guest::wire::Edges {
                                                                                    top: (13.0) as f32,
                                                                                    right: (14.0) as f32,
                                                                                    bottom: (13.0) as f32,
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
                                                                                            key: format!("{}/@container:942", for_scope),
                                                                                            width: Some(
                                                                                                ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
                                                                                            ),
                                                                                            height: Some(
                                                                                                ::ducktape_view_guest::wire::Length::Fixed((34.0) as f32),
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
                                                                                                    font: None,
                                                                                                },
                                                                                                key: format!("{}/@text:950", for_scope),
                                                                                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[38]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: true,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: (agent.initials.to_owned()).to_string(),
                                                                                            }),
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
                                                                                                            key: format!("{}/@text:962", for_scope),
                                                                                                            size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[4]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: false,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (agent.name.to_owned()).to_string(),
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
                                                                                                            key: format!("{}/@container:967", for_scope),
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
                                                                                                                    wrapping: None,
                                                                                                                    tracking: 0.0f32,
                                                                                                                    font: None,
                                                                                                                },
                                                                                                                key: format!("{}/@text:973", for_scope),
                                                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                color: Some(palette.colors[13]),
                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                    monospace: true,
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                                },
                                                                                                                width: None,
                                                                                                                align_x: None,
                                                                                                                content: (agent.capability.to_owned()).to_string(),
                                                                                                            }),
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:957", for_scope),
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
                                                                                                            key: format!("{}/@text:986", for_scope),
                                                                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[71]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (crate::host::skill_count(
                                                                                                                ::std::convert::AsRef::as_ref(&(agent.skills)),
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
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:992", for_scope),
                                                                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[71]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: ("skills · owner".to_owned()).to_string(),
                                                                                                        });
                                                                                                    if (agent.owner_handle).is_empty() {
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
                                                                                                                key: format!("{}/@text:999", for_scope),
                                                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                color: Some(palette.colors[72]),
                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                    monospace: true,
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                                },
                                                                                                                width: None,
                                                                                                                align_x: None,
                                                                                                                content: ("unowned".to_owned()).to_string(),
                                                                                                            });
                                                                                                    }
                                                                                                    if (!(agent.owner_handle).is_empty()) {
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
                                                                                                                key: format!("{}/@text:1006", for_scope),
                                                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                color: Some(palette.colors[72]),
                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                    monospace: true,
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                                },
                                                                                                                width: None,
                                                                                                                align_x: None,
                                                                                                                content: ("@".to_owned()).to_string(),
                                                                                                            });
                                                                                                    }
                                                                                                    if (!(agent.owner_handle).is_empty()) {
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
                                                                                                                key: format!("{}/@text:1013", for_scope),
                                                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                                color: Some(palette.colors[72]),
                                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                                    monospace: true,
                                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                                                },
                                                                                                                width: None,
                                                                                                                align_x: None,
                                                                                                                content: (agent.owner_handle.to_owned()).to_string(),
                                                                                                            });
                                                                                                    }
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:981", for_scope),
                                                                                                        wrap: None,
                                                                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                                        spacing: Some((5.0) as f32),
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
                                                                                                key: format!("{}/@layout:956", for_scope),
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
                                                                                    if (agent.status == "active") {
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
                                                                                                key: format!("{}/@container:1022", for_scope),
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
                                                                                                background: (Some(palette.colors[27]))
                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[28]),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
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
                                                                                                            key: format!("{}/@container:1031", for_scope),
                                                                                                            width: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
                                                                                                            ),
                                                                                                            height: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
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
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
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
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:1038", for_scope),
                                                                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[25]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: ("ACTIVE".to_owned()).to_string(),
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:1030", for_scope),
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
                                                                                            });
                                                                                    }
                                                                                    if (agent.status == "paused") {
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
                                                                                                key: format!("{}/@container:1045", for_scope),
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
                                                                                                background: (Some(palette.colors[32]))
                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[33]),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
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
                                                                                                            key: format!("{}/@container:1054", for_scope),
                                                                                                            width: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
                                                                                                            ),
                                                                                                            height: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
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
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
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
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:1061", for_scope),
                                                                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[30]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: ("PAUSED".to_owned()).to_string(),
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:1053", for_scope),
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
                                                                                            });
                                                                                    }
                                                                                    if ((agent.status != "active")
                                                                                        && (agent.status != "paused"))
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
                                                                                                key: format!("{}/@container:1068", for_scope),
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
                                                                                                background: (Some(palette.colors[32]))
                                                                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[33]),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((6.0) as f32).max(0.0).min(f32::MAX),
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
                                                                                                            key: format!("{}/@container:1077", for_scope),
                                                                                                            width: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
                                                                                                            ),
                                                                                                            height: Some(
                                                                                                                ::ducktape_view_guest::wire::Length::Fixed((5.0) as f32),
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
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
                                                                                                                    ((2.5) as f32).max(0.0).min(f32::MAX),
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
                                                                                                                line_height: None,
                                                                                                                shaping: None,
                                                                                                                wrapping: None,
                                                                                                                tracking: 0.0f32,
                                                                                                                font: None,
                                                                                                            },
                                                                                                            key: format!("{}/@text:1084", for_scope),
                                                                                                            size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                            color: Some(palette.colors[30]),
                                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                                monospace: true,
                                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                            },
                                                                                                            width: None,
                                                                                                            align_x: None,
                                                                                                            content: (agent.status.to_owned()).to_string(),
                                                                                                        });
                                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                                        max_width: None,
                                                                                                        clip: false,
                                                                                                        key: format!("{}/@layout:1076", for_scope),
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
                                                                                            });
                                                                                    }
                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                        max_width: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@layout:937", for_scope),
                                                                                        wrap: None,
                                                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                        spacing: Some((13.0) as f32),
                                                                                        padding: None,
                                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                        height: None,
                                                                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                        background: None,
                                                                                        border: None,
                                                                                        children: children,
                                                                                    }
                                                                                }),
                                                                            }),
                                                                        ),
                                                                        label: Some(String::from(agent.name.to_owned())),
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(
                                                                                Message::OpenAgent(agent.id.to_owned()),
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
                                                                                background: Some(palette.colors[2]),
                                                                                text: None,
                                                                                border: None,
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
                                                                        key: format!("{}/@container:1092", for_scope),
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
                                                                    key: format!("{}/@layout:924", for_scope),
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
                                                    }
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:915", node_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                        spacing: Some((11.0) as f32),
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (18.0) as f32,
                                                            right: (18.0) as f32,
                                                            bottom: (18.0) as f32,
                                                            left: (18.0) as f32,
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
                                if ((!(self.selected).is_empty()) || self.creating) {
                                    children
                                        .push({
                                            let node_scope = format!("{}/editor-resize", node_scope);
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
                                                            let route = move |delta: (f64, f64)| Message::EditorResized(
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
                                                    let node_scope = format!("{}/editor-divider", node_scope);
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
                                        .push({
                                            let node_scope = format!("{}/editor", node_scope);
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
                                                        (self.editor_width) as f32,
                                                    ),
                                                ),
                                                height: Some(::ducktape_view_guest::wire::Length::Fill),
                                                padding: None,
                                                align_x: None,
                                                align_y: None,
                                                background: (Some(palette.colors[3]))
                                                    .map(::ducktape_view_guest::wire::Background::Color),
                                                border: Some(::ducktape_view_guest::wire::Border {
                                                    color: Some(palette.colors[39]),
                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                    radius: None,
                                                }),
                                                snap: None,
                                                content: Box::new(::ducktape_view_guest::wire::Node::Scroll {
                                                    on_scroll: None,
                                                    virtual_rows: false,
                                                    key: format!("{}/@layout:1112", node_scope),
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
                                                                if self.creating {
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
                                                                            key: format!("{}/@text:1128", node_scope),
                                                                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[4]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: ("New agent".to_owned()).to_string(),
                                                                        });
                                                                }
                                                                if (!self.creating) {
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
                                                                            key: format!("{}/@text:1134", node_scope),
                                                                            size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[4]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: (self.draft_name.to_owned()).to_string(),
                                                                        });
                                                                }
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Space {
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        height: None,
                                                                    });
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                        checked: None,
                                                                        expanded: None,
                                                                        description: None,
                                                                        key: format!("{}/@button:1140", node_scope),
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
                                                                                key: format!("{}/@text:1146", node_scope),
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
                                                                        label: Some(String::from("Close editor".to_owned())),
                                                                        on_press: Some(
                                                                            ::ducktape_view_guest::slots::message(Message::CloseEditor),
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
                                                                            active: ::ducktape_view_guest::wire::Face::default(),
                                                                            hovered: None,
                                                                            pressed: None,
                                                                            disabled: None,
                                                                        },
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:1122", node_scope),
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
                                                        if ((!self.can_edit) && (!self.creating)) {
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
                                                                    key: format!("{}/@container:1148", node_scope),
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
                                                                    background: (Some(palette.colors[32]))
                                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some(palette.colors[33]),
                                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
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
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        },
                                                                        key: format!("{}/@text:1157", node_scope),
                                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[30]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                        align_x: None,
                                                                        content: ("Only this agent's controller can change its record. You are reading it."
                                                                            .to_owned())
                                                                            .to_string(),
                                                                    }),
                                                                });
                                                        }
                                                        if (!self.creating) {
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
                                                                            key: format!("{}/@text:1170", node_scope),
                                                                            size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[4]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: ("Standing".to_owned()).to_string(),
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
                                                                                wrapping: None,
                                                                                tracking: 0.0f32,
                                                                                font: None,
                                                                            },
                                                                            key: format!("{}/@text:1176", node_scope),
                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[71]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: true,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: (self.selected_status.to_owned()).to_string(),
                                                                        });
                                                                    if (self.can_edit && (self.selected_status == "active")) {
                                                                        children
                                                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                                                checked: None,
                                                                                expanded: None,
                                                                                description: None,
                                                                                key: format!("{}/@button:1182", node_scope),
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
                                                                                        key: format!("{}/@text:1188", node_scope),
                                                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: None,
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: ("Pause".to_owned()).to_string(),
                                                                                    }),
                                                                                ),
                                                                                label: Some(String::from("Pause agent".to_owned())),
                                                                                on_press: Some(
                                                                                    ::ducktape_view_guest::slots::message(
                                                                                        Message::SetStatus(self.selected.to_owned(), true),
                                                                                    ),
                                                                                ),
                                                                                width: None,
                                                                                height: Some(
                                                                                    ::ducktape_view_guest::wire::Length::Fixed((26.0) as f32),
                                                                                ),
                                                                                padding: Some(
                                                                                    ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
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
                                                                    }
                                                                    if (self.can_edit && (self.selected_status == "paused")) {
                                                                        children
                                                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                                                checked: None,
                                                                                expanded: None,
                                                                                description: None,
                                                                                key: format!("{}/@button:1190", node_scope),
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
                                                                                        key: format!("{}/@text:1196", node_scope),
                                                                                        size: Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: None,
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: false,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: ("Resume".to_owned()).to_string(),
                                                                                    }),
                                                                                ),
                                                                                label: Some(String::from("Resume agent".to_owned())),
                                                                                on_press: Some(
                                                                                    ::ducktape_view_guest::slots::message(
                                                                                        Message::SetStatus(self.selected.to_owned(), false),
                                                                                    ),
                                                                                ),
                                                                                width: None,
                                                                                height: Some(
                                                                                    ::ducktape_view_guest::wire::Length::Fixed((26.0) as f32),
                                                                                ),
                                                                                padding: Some(
                                                                                    ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
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
                                                                    }
                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                        max_width: None,
                                                                        clip: false,
                                                                        key: format!("{}/@layout:1165", node_scope),
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
                                                                        key: format!("{}/@text:1200", node_scope),
                                                                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[4]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: ("Identity".to_owned()).to_string(),
                                                                    });
                                                                if self.creating {
                                                                    children
                                                                        .push({
                                                                            let node_scope = format!("{}/agent-id", node_scope);
                                                                            ::ducktape_view_guest::wire::Node::Input {
                                                                                options: ::ducktape_view_guest::wire::InputOptions {
                                                                                    label: ("Agent id".to_owned()).to_string(),
                                                                                    description: None,
                                                                                    disabled: false,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
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
                                                                                    "a-dns-label, e.g. chiefduck".to_owned(),
                                                                                ),
                                                                                value: (self.draft_id).to_string(),
                                                                                on_input: ::ducktape_view_guest::slots::handler::<
                                                                                    String,
                                                                                    Message,
                                                                                >(
                                                                                    Box::new({
                                                                                        let route = Message::BindDraftId as fn(String) -> Message;
                                                                                        move |sent: String| Some(route(sent))
                                                                                    }),
                                                                                ),
                                                                                on_submit: None,
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
                                                                                        background: Some(palette.colors[55]),
                                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                                            color: Some({
                                                                                                let mut color = palette.colors[4];
                                                                                                color.0[3] = 0.160000;
                                                                                                color
                                                                                            }),
                                                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                            radius: Some([
                                                                                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ]),
                                                                                        }),
                                                                                        value: Some(palette.colors[4]),
                                                                                        placeholder: Some(palette.colors[5]),
                                                                                        selection: Some({
                                                                                            let mut color = palette.colors[4];
                                                                                            color.0[3] = 0.180000;
                                                                                            color
                                                                                        }),
                                                                                    },
                                                                                    hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                        icon: None,
                                                                                        background: Some(palette.colors[55]),
                                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                                            color: Some({
                                                                                                let mut color = palette.colors[4];
                                                                                                color.0[3] = 0.210000;
                                                                                                color
                                                                                            }),
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
                                                                                        background: Some({
                                                                                            let mut color = palette.colors[6];
                                                                                            color.0[3] = 0.540000;
                                                                                            color
                                                                                        }),
                                                                                        border: None,
                                                                                        value: Some(palette.colors[5]),
                                                                                        placeholder: None,
                                                                                        selection: None,
                                                                                    }),
                                                                                }),
                                                                            }
                                                                        });
                                                                }
                                                                if ((self.creating && (!(self.draft_id).is_empty()))
                                                                    && (!crate::host::valid_agent_id(
                                                                        ::std::convert::AsRef::as_ref(&(self.draft_id)),
                                                                    )))
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
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:1219", node_scope),
                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[20]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                            align_x: None,
                                                                            content: ("An agent id is a lowercase DNS label: a-z, 0-9 and hyphens, no hyphen at either end."
                                                                                .to_owned())
                                                                                .to_string(),
                                                                        });
                                                                }
                                                                if (!self.creating) {
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
                                                                            key: format!("{}/@text:1225", node_scope),
                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[71]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: true,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: (self.draft_id.to_owned()).to_string(),
                                                                        });
                                                                }
                                                                children
                                                                    .push({
                                                                        let node_scope = format!("{}/agent-name", node_scope);
                                                                        ::ducktape_view_guest::wire::Node::Input {
                                                                            options: ::ducktape_view_guest::wire::InputOptions {
                                                                                label: ("Display name".to_owned()).to_string(),
                                                                                description: None,
                                                                                disabled: (!self.can_edit),
                                                                                padding: Some(
                                                                                    ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
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
                                                                            placeholder: String::from("display name…".to_owned()),
                                                                            value: (self.draft_name).to_string(),
                                                                            on_input: ::ducktape_view_guest::slots::handler::<
                                                                                String,
                                                                                Message,
                                                                            >(
                                                                                Box::new({
                                                                                    let route = Message::BindDraftName as fn(String) -> Message;
                                                                                    move |sent: String| Some(route(sent))
                                                                                }),
                                                                            ),
                                                                            on_submit: None,
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
                                                                                    background: Some(palette.colors[55]),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: Some({
                                                                                            let mut color = palette.colors[4];
                                                                                            color.0[3] = 0.160000;
                                                                                            color
                                                                                        }),
                                                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                        radius: Some([
                                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                        ]),
                                                                                    }),
                                                                                    value: Some(palette.colors[4]),
                                                                                    placeholder: Some(palette.colors[5]),
                                                                                    selection: Some({
                                                                                        let mut color = palette.colors[4];
                                                                                        color.0[3] = 0.180000;
                                                                                        color
                                                                                    }),
                                                                                },
                                                                                hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                    icon: None,
                                                                                    background: Some(palette.colors[55]),
                                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                                        color: Some({
                                                                                            let mut color = palette.colors[4];
                                                                                            color.0[3] = 0.210000;
                                                                                            color
                                                                                        }),
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
                                                                                    background: Some({
                                                                                        let mut color = palette.colors[6];
                                                                                        color.0[3] = 0.540000;
                                                                                        color
                                                                                    }),
                                                                                    border: None,
                                                                                    value: Some(palette.colors[5]),
                                                                                    placeholder: None,
                                                                                    selection: None,
                                                                                }),
                                                                            }),
                                                                        }
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:1199", node_scope),
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
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        },
                                                                        key: format!("{}/@text:1247", node_scope),
                                                                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[4]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: ("Executor".to_owned()).to_string(),
                                                                    });
                                                                if self.can_edit {
                                                                    children
                                                                        .push({
                                                                            let node_scope = format!("{}/agent-capability", node_scope);
                                                                            {
                                                                                let options = crate::host::capability_options(
                                                                                    ::std::convert::AsRef::as_ref(&(self.capabilities)),
                                                                                    ::std::convert::AsRef::as_ref(
                                                                                        &(crate::host::or_empty(
                                                                                            ::std::borrow::Borrow::borrow(&(self.draft_capability)),
                                                                                        )),
                                                                                    ),
                                                                                );
                                                                                let selected = self.draft_capability.clone();
                                                                                ::ducktape_view_guest::wire::Node::PickList {
                                                                                    key: node_scope.clone(),
                                                                                    options: options
                                                                                        .iter()
                                                                                        .map(|option| option.to_string())
                                                                                        .collect(),
                                                                                    selected: selected
                                                                                        .as_ref()
                                                                                        .and_then(|chosen| {
                                                                                            options.iter().position(|option| option == chosen)
                                                                                        })
                                                                                        .map(|index| index as u32),
                                                                                    placeholder: Some(
                                                                                        ("pick a capability…".to_owned()).to_string(),
                                                                                    ),
                                                                                    on_select: ::ducktape_view_guest::slots::handler::<
                                                                                        u32,
                                                                                        Message,
                                                                                    >(
                                                                                        Box::new({
                                                                                            let route = move |value| Message::PickCapabilityOption(
                                                                                                value,
                                                                                            );
                                                                                            let table: Vec<Message> = options
                                                                                                .iter()
                                                                                                .cloned()
                                                                                                .map(route)
                                                                                                .collect();
                                                                                            move |sent: u32| table.get(sent as usize).cloned()
                                                                                        }),
                                                                                    ),
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    style: ::ducktape_view_guest::wire::PickListStyle {
                                                                                        active: None,
                                                                                        hovered: None,
                                                                                        opened: None,
                                                                                        opened_hovered: None,
                                                                                        menu: None,
                                                                                    },
                                                                                    settings: Box::new(::ducktape_view_guest::wire::PickOptions {
                                                                                        menu_height: None,
                                                                                        padding: None,
                                                                                        text_size: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        font: None,
                                                                                        handle: None,
                                                                                        on_open: None,
                                                                                        on_close: None,
                                                                                    }),
                                                                                }
                                                                            }
                                                                        });
                                                                }
                                                                if (!self.can_edit) {
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
                                                                            key: format!("{}/@text:1258", node_scope),
                                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[4]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: true,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: (crate::host::or_empty(
                                                                                ::std::borrow::Borrow::borrow(&(self.draft_capability)),
                                                                            ))
                                                                                .to_string(),
                                                                        });
                                                                }
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:1246", node_scope),
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
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        },
                                                                        key: format!("{}/@text:1267", node_scope),
                                                                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[4]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: ("Skills".to_owned()).to_string(),
                                                                    });
                                                                for (index, skill) in self.draft_skills.iter().enumerate() {
                                                                    let for_scope = format!(
                                                                        "{}/@for:1791({})", node_scope, index
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
                                                                                            key: format!("{}/@text:1275", for_scope),
                                                                                            size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[4]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: false,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: (skill.name.to_owned()).to_string(),
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
                                                                                            key: format!("{}/@text:1280", for_scope),
                                                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                            color: Some(palette.colors[72]),
                                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                                monospace: true,
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            },
                                                                                            width: None,
                                                                                            align_x: None,
                                                                                            content: (skill.source_prefix.to_owned()).to_string(),
                                                                                        });
                                                                                    if (!(skill.source_snapshot).is_empty()) {
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
                                                                                                key: format!("{}/@text:1286", for_scope),
                                                                                                size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: Some(palette.colors[72]),
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: true,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: (skill.source_snapshot.to_owned()).to_string(),
                                                                                            });
                                                                                    }
                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                        max_width: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@layout:1274", for_scope),
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
                                                                            if (self.can_edit && skill.always) {
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:1292", for_scope),
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
                                                                                                key: format!("{}/@text:1298", for_scope),
                                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: None,
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: (crate::host::skill_mode(skill.always)).to_string(),
                                                                                            }),
                                                                                        ),
                                                                                        label: Some(String::from("Load on demand".to_owned())),
                                                                                        on_press: Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                Message::LoadSkill(skill.name.to_owned(), false),
                                                                                            ),
                                                                                        ),
                                                                                        width: None,
                                                                                        height: Some(
                                                                                            ::ducktape_view_guest::wire::Length::Fixed((24.0) as f32),
                                                                                        ),
                                                                                        padding: Some(
                                                                                            ::ducktape_view_guest::wire::Edges::all((4.0) as f32),
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
                                                                            }
                                                                            if (self.can_edit && (!skill.always)) {
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:1300", for_scope),
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
                                                                                                key: format!("{}/@text:1306", for_scope),
                                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                color: None,
                                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                                    monospace: false,
                                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                },
                                                                                                width: None,
                                                                                                align_x: None,
                                                                                                content: (crate::host::skill_mode(skill.always)).to_string(),
                                                                                            }),
                                                                                        ),
                                                                                        label: Some(String::from("Load always".to_owned())),
                                                                                        on_press: Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                Message::LoadSkill(skill.name.to_owned(), true),
                                                                                            ),
                                                                                        ),
                                                                                        width: None,
                                                                                        height: Some(
                                                                                            ::ducktape_view_guest::wire::Length::Fixed((24.0) as f32),
                                                                                        ),
                                                                                        padding: Some(
                                                                                            ::ducktape_view_guest::wire::Edges::all((4.0) as f32),
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
                                                                            }
                                                                            if (!self.can_edit) {
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
                                                                                        key: format!("{}/@text:1308", for_scope),
                                                                                        size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                        color: Some(palette.colors[71]),
                                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                                            monospace: true,
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                        },
                                                                                        width: None,
                                                                                        align_x: None,
                                                                                        content: (crate::host::skill_mode(skill.always)).to_string(),
                                                                                    });
                                                                            }
                                                                            if self.can_edit {
                                                                                children
                                                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                                                        checked: None,
                                                                                        expanded: None,
                                                                                        description: None,
                                                                                        key: format!("{}/@button:1314", for_scope),
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
                                                                                                key: format!("{}/@text:1320", for_scope),
                                                                                                size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
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
                                                                                        label: Some(String::from("Remove skill".to_owned())),
                                                                                        on_press: Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                Message::RemoveSkill(skill.name.to_owned()),
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
                                                                                key: format!("{}/@layout:1273", for_scope),
                                                                                wrap: None,
                                                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                spacing: Some((6.0) as f32),
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
                                                                if self.can_edit {
                                                                    children
                                                                        .push({
                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                            children
                                                                                .push({
                                                                                    let node_scope = format!("{}/skill-name", node_scope);
                                                                                    ::ducktape_view_guest::wire::Node::Input {
                                                                                        options: ::ducktape_view_guest::wire::InputOptions {
                                                                                            label: ("Skill name".to_owned()).to_string(),
                                                                                            description: None,
                                                                                            disabled: false,
                                                                                            padding: Some(
                                                                                                ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
                                                                                            ),
                                                                                            text_size: Some((12.5) as f32),
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
                                                                                            "skill name (its mount directory)…".to_owned(),
                                                                                        ),
                                                                                        value: (self.skill_name).to_string(),
                                                                                        on_input: ::ducktape_view_guest::slots::handler::<
                                                                                            String,
                                                                                            Message,
                                                                                        >(
                                                                                            Box::new({
                                                                                                let route = Message::BindSkillName as fn(String) -> Message;
                                                                                                move |sent: String| Some(route(sent))
                                                                                            }),
                                                                                        ),
                                                                                        on_submit: None,
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
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.160000;
                                                                                                        color
                                                                                                    }),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ]),
                                                                                                }),
                                                                                                value: Some(palette.colors[4]),
                                                                                                placeholder: Some(palette.colors[5]),
                                                                                                selection: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.180000;
                                                                                                    color
                                                                                                }),
                                                                                            },
                                                                                            hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.210000;
                                                                                                        color
                                                                                                    }),
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
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[6];
                                                                                                    color.0[3] = 0.540000;
                                                                                                    color
                                                                                                }),
                                                                                                border: None,
                                                                                                value: Some(palette.colors[5]),
                                                                                                placeholder: None,
                                                                                                selection: None,
                                                                                            }),
                                                                                        }),
                                                                                    }
                                                                                });
                                                                            children
                                                                                .push({
                                                                                    let node_scope = format!("{}/skill-prefix", node_scope);
                                                                                    ::ducktape_view_guest::wire::Node::Input {
                                                                                        options: ::ducktape_view_guest::wire::InputOptions {
                                                                                            label: ("Skill source prefix".to_owned()).to_string(),
                                                                                            description: None,
                                                                                            disabled: false,
                                                                                            padding: Some(
                                                                                                ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
                                                                                            ),
                                                                                            text_size: Some((12.5) as f32),
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
                                                                                            "/shared/skills/<name> when left empty".to_owned(),
                                                                                        ),
                                                                                        value: (self.skill_prefix).to_string(),
                                                                                        on_input: ::ducktape_view_guest::slots::handler::<
                                                                                            String,
                                                                                            Message,
                                                                                        >(
                                                                                            Box::new({
                                                                                                let route = Message::BindSkillPrefix
                                                                                                    as fn(String) -> Message;
                                                                                                move |sent: String| Some(route(sent))
                                                                                            }),
                                                                                        ),
                                                                                        on_submit: None,
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
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.160000;
                                                                                                        color
                                                                                                    }),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ]),
                                                                                                }),
                                                                                                value: Some(palette.colors[4]),
                                                                                                placeholder: Some(palette.colors[5]),
                                                                                                selection: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.180000;
                                                                                                    color
                                                                                                }),
                                                                                            },
                                                                                            hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.210000;
                                                                                                        color
                                                                                                    }),
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
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[6];
                                                                                                    color.0[3] = 0.540000;
                                                                                                    color
                                                                                                }),
                                                                                                border: None,
                                                                                                value: Some(palette.colors[5]),
                                                                                                placeholder: None,
                                                                                                selection: None,
                                                                                            }),
                                                                                        }),
                                                                                    }
                                                                                });
                                                                            children
                                                                                .push({
                                                                                    let node_scope = format!("{}/skill-snapshot", node_scope);
                                                                                    ::ducktape_view_guest::wire::Node::Input {
                                                                                        options: ::ducktape_view_guest::wire::InputOptions {
                                                                                            label: ("Skill snapshot pin".to_owned()).to_string(),
                                                                                            description: None,
                                                                                            disabled: false,
                                                                                            padding: Some(
                                                                                                ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
                                                                                            ),
                                                                                            text_size: Some((12.5) as f32),
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
                                                                                            "snapshot id to pin (optional)".to_owned(),
                                                                                        ),
                                                                                        value: (self.skill_snapshot).to_string(),
                                                                                        on_input: ::ducktape_view_guest::slots::handler::<
                                                                                            String,
                                                                                            Message,
                                                                                        >(
                                                                                            Box::new({
                                                                                                let route = Message::BindSkillSnapshot
                                                                                                    as fn(String) -> Message;
                                                                                                move |sent: String| Some(route(sent))
                                                                                            }),
                                                                                        ),
                                                                                        on_submit: None,
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
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.160000;
                                                                                                        color
                                                                                                    }),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ]),
                                                                                                }),
                                                                                                value: Some(palette.colors[4]),
                                                                                                placeholder: Some(palette.colors[5]),
                                                                                                selection: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.180000;
                                                                                                    color
                                                                                                }),
                                                                                            },
                                                                                            hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.210000;
                                                                                                        color
                                                                                                    }),
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
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[6];
                                                                                                    color.0[3] = 0.540000;
                                                                                                    color
                                                                                                }),
                                                                                                border: None,
                                                                                                value: Some(palette.colors[5]),
                                                                                                placeholder: None,
                                                                                                selection: None,
                                                                                            }),
                                                                                        }),
                                                                                    }
                                                                                });
                                                                            children
                                                                                .push({
                                                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                                    children
                                                                                        .push({
                                                                                            let node_scope = format!("{}/skill-always", node_scope);
                                                                                            ::ducktape_view_guest::wire::Node::Toggle {
                                                                                                key: node_scope.clone(),
                                                                                                kind: ::ducktape_view_guest::wire::ToggleKind::Checkbox,
                                                                                                label: ("load always (persona)".to_owned()).to_string(),
                                                                                                checked: (self.skill_always),
                                                                                                on_toggle: Some(
                                                                                                    ::ducktape_view_guest::slots::handler::<
                                                                                                        bool,
                                                                                                        Message,
                                                                                                    >(
                                                                                                        Box::new({
                                                                                                            let route = move |value| Message::SetSkillAlways(value);
                                                                                                            let on = route(true);
                                                                                                            let off = route(false);
                                                                                                            move |sent: bool| Some(
                                                                                                                if sent { on.clone() } else { off.clone() },
                                                                                                            )
                                                                                                        }),
                                                                                                    ),
                                                                                                ),
                                                                                                width: None,
                                                                                                style: ::ducktape_view_guest::wire::ToggleStyle {
                                                                                                    tone: None,
                                                                                                    active_on: None,
                                                                                                    active_off: None,
                                                                                                    hovered_on: None,
                                                                                                    hovered_off: None,
                                                                                                    disabled_on: None,
                                                                                                    disabled_off: None,
                                                                                                },
                                                                                            }
                                                                                        });
                                                                                    children
                                                                                        .push(::ducktape_view_guest::wire::Node::Space {
                                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                            height: None,
                                                                                        });
                                                                                    children
                                                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                                                            checked: None,
                                                                                            expanded: None,
                                                                                            description: None,
                                                                                            key: format!("{}/@button:1362", node_scope),
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
                                                                                                    key: format!("{}/@text:1369", node_scope),
                                                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                                    color: None,
                                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                                        monospace: false,
                                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                    },
                                                                                                    width: None,
                                                                                                    align_x: None,
                                                                                                    content: ("Add skill".to_owned()).to_string(),
                                                                                                }),
                                                                                            ),
                                                                                            label: Some(String::from("Add skill".to_owned())),
                                                                                            on_press: if (((self.skill_name).trim().to_owned())
                                                                                                .is_empty())
                                                                                            {
                                                                                                None
                                                                                            } else {
                                                                                                Some(
                                                                                                        ::ducktape_view_guest::slots::message(Message::AddSkill),
                                                                                                    )
                                                                                            },
                                                                                            width: None,
                                                                                            height: Some(
                                                                                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                                                                                            ),
                                                                                            padding: Some(
                                                                                                ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
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
                                                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                                                        max_width: None,
                                                                                        clip: false,
                                                                                        key: format!("{}/@layout:1359", node_scope),
                                                                                        wrap: None,
                                                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                        spacing: Some((6.0) as f32),
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
                                                                                key: format!("{}/@layout:1322", node_scope),
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
                                                                        });
                                                                }
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:1266", node_scope),
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
                                                            });
                                                        if (self.can_edit && (!self.creating)) {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: None,
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:1372", node_scope),
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
                                                                            key: format!("{}/@text:1379", node_scope),
                                                                            size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: None,
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: ("Save".to_owned()).to_string(),
                                                                        }),
                                                                    ),
                                                                    label: Some(String::from("Save agent".to_owned())),
                                                                    on_press: if ((((self.draft_name).trim().to_owned())
                                                                        .is_empty()
                                                                        || (crate::host::or_empty(
                                                                            ::std::borrow::Borrow::borrow(&(self.draft_capability)),
                                                                        ))
                                                                            .is_empty()))
                                                                    {
                                                                        None
                                                                    } else {
                                                                        Some(
                                                                                ::ducktape_view_guest::slots::message(Message::SubmitSave),
                                                                            )
                                                                    },
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((10.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[7]),
                                                                                text: Some(palette.colors[9]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: None,
                                                                                    width: None,
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[8]),
                                                                            pressed_background: Some({
                                                                                let mut color = palette.colors[7];
                                                                                color.0[3] = 0.800000;
                                                                                color
                                                                            }),
                                                                            disabled_background: Some(palette.colors[10]),
                                                                            disabled_text: Some(palette.colors[11]),
                                                                            disabled_opacity: None,
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
                                                        if self.creating {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: None,
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:1381", node_scope),
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
                                                                            key: format!("{}/@text:1388", node_scope),
                                                                            size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: None,
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: ("Register".to_owned()).to_string(),
                                                                        }),
                                                                    ),
                                                                    label: Some(String::from("Register agent".to_owned())),
                                                                    on_press: if ((((!crate::host::valid_agent_id(
                                                                        ::std::convert::AsRef::as_ref(&(self.draft_id)),
                                                                    )) || ((self.draft_name).trim().to_owned()).is_empty())
                                                                        || (crate::host::or_empty(
                                                                            ::std::borrow::Borrow::borrow(&(self.draft_capability)),
                                                                        ))
                                                                            .is_empty()))
                                                                    {
                                                                        None
                                                                    } else {
                                                                        Some(
                                                                                ::ducktape_view_guest::slots::message(
                                                                                    Message::SubmitRegister,
                                                                                ),
                                                                            )
                                                                    },
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((10.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[7]),
                                                                                text: Some(palette.colors[9]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: None,
                                                                                    width: None,
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[8]),
                                                                            pressed_background: Some({
                                                                                let mut color = palette.colors[7];
                                                                                color.0[3] = 0.800000;
                                                                                color
                                                                            }),
                                                                            disabled_background: Some(palette.colors[10]),
                                                                            disabled_text: Some(palette.colors[11]),
                                                                            disabled_opacity: None,
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
                                                            key: format!("{}/@layout:1117", node_scope),
                                                            wrap: None,
                                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                                            spacing: Some((14.0) as f32),
                                                            padding: Some(::ducktape_view_guest::wire::Edges {
                                                                top: (18.0) as f32,
                                                                right: (18.0) as f32,
                                                                bottom: (18.0) as f32,
                                                                left: (18.0) as f32,
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
                                }
                                ::ducktape_view_guest::wire::Node::Linear {
                                    max_width: None,
                                    clip: false,
                                    key: format!("{}/@layout:908", node_scope),
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
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:336", node_scope),
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
    AgentsView, "Agents",
    "The registry of who may act, on which executor, and what their runs did.",
    ["agents"]
);
