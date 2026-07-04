# Ducktape Invitation Protocol — Deep Dive

> 노드 초대(invitation) 프로토콜의 구조 분석. `bin/node`(CLI + joiner 런타임), `crates/system`(on-chain admission),
> `crates/kernel/consensus`(epoch cutover), `app/src-tauri`(desktop 래퍼)를 가로질러 추적한 결과.
> 모든 `path:line` 참조는 분석 시점 기준.

## 0. 한 줄 요약

Ducktape의 초대는 **하나의 프로토콜이 아니라 genesis를 기준으로 갈라지는 두 개의 admission 경로**다.
genesis 전에는 초대가 **파일 편집**(founder가 자기 `network.toml`에 키를 추가), genesis 후에는
**on-chain governance 투표**(`AddValidator` proposal → epoch cutover)로 이뤄진다.
둘을 잇는 공통 매개체가 `ducktape-invite-v2:` 라는 base64url 한 줄짜리 **invite blob**이다.

```
                       ┌─────────────────── genesis 경계 (storage/ 생성 시점) ───────────────────┐
  keygen → init ──────►│ invite ⇄ join ⇄ admit (파일 편집, 반복)  │  invite-accept (governance)  │
  (founder)            │  descriptor 확정 → 첫 run_node = FREEZE   │  joiner park → statesync →   │
                       │                                          │  self-promote (re-exec)      │
                       └──────────────────────────────────────────┴──────────────────────────────┘
```

핵심 파일:

- `bin/node/src/config.rs` — wire format + `NetworkDescriptor` + genesis fingerprint
- `bin/node/src/main.rs` — CLI verbs + joiner 런타임
- `crates/system/{governance,valset,valset-mesh-interface}` — on-chain admission
- `crates/kernel/consensus/src/valset_orchestrator.rs` — epoch cutover 스케줄링
- `app/src-tauri/src/workspaces.rs` — desktop 래퍼

---

## 1. Wire format — invite blob v2 (`config.rs:363-510`)

blob은 `ducktape-invite-v2:` + base64url(`URL_SAFE_NO_PAD`)로 감싼 **length-prefixed 바이너리**다.
v1은 `network.toml` 전체를 hex로 감싸 470자를 넘겼는데, v2는 joiner가 실제로 필요한 것만 담아 ~4배 작아졌고
**flag-day 교체**(v1 blob은 더 이상 decode 안 됨).

`pack_invite` (`config.rs:399-431`)가 만드는 바이트 레이아웃:

```
[1] version = 2
[1] chain_id 길이  +  [n] chain_id  (ascii, 예: "ducktape#a1b2c3d4")
[1] validator 개수 +  [32×k] raw ed25519 pubkey들  (hex 아님, dedup됨)
[1] bootstrap 개수 +  각 항목: [32] raw pubkey + [1] "host:port" 길이 + [m] host:port
```

설계상 특징:

- **plumbing은 안 실린다** — 포트/storage/RPC/http 설정은 각 노드의 `node.toml` 소관이지 공유 대상이 아니다.
- **scheme은 암묵적** (ed25519 전용) — 전송도 저장도 안 함.
- bootstrap의 `host:port`는 **IP 리터럴 또는 hostname 그대로** 저장 → `pubkey@node.example.com:443`이
  round-trip되고 **DNS 해석은 dial 시점**에 일어난다 (`config.rs:421-422`).
- `unpack_invite` (`config.rs:436-472`)는 `InviteReader`라는 bounds-checked cursor로 파싱 —
  truncation/overflow/trailing-byte를 모두 loud하게 거부한다. decode 결과를 **`from_toml`과 동일하게
  canonicalize**(lowercase, sorted)해서 genesis fingerprint가 founder 것과 bit-identical하게 나오도록 보장한다.

---

## 2. NetworkDescriptor + genesis fingerprint — 프로토콜의 보안 앵커 (`config.rs:136-278`)

```rust
struct NetworkDescriptor {
    chain_id: String,        // "name#salt", namespace 겸용
    scheme: String,          // 반드시 ed25519
    validators: Vec<String>, // genesis validator 집합 (SET, 정렬 유지)
    bootstrap: Vec<String>,  // "hexpubkey@host:port" dial 힌트 (advisory)
}
```

두 가지가 이 프로토콜의 안전성을 지탱한다:

**(a) `mint_chain_id` (`config.rs:119-130`)** — `SHA256(initiator_pubkey ‖ wall_clock_nanos)`의 앞 4바이트를
salt로 붙인다. 같은 이름의 무관한 네트워크가 namespace 충돌 없이 갈라지게 하되, RNG 배선 없이 유니크한
id를 만든다. **재생성 불가** → 실수로 clobber되면 복구 불가(그래서 아래 guard가 중요).

**(b) `genesis_namespace` (`config.rs:210-233`)** — 진짜 핵심.
`SHA256("ducktape:genesis:v1:" ‖ scheme ‖ 정렬된 validator들)`의 **128비트**를 chain_id에 붙인 fingerprint다.
이 namespace가 discovery handshake / simplex 합의 scheme / epoch genesis floor를 domain-separate한다. 결과적으로:

> **stale descriptor를 든 노드(예: pre-genesis `admit`을 놓쳐 옛 validator 목록 유지)는 아예 연결조차 못 한다 —
> genesis 불일치가 silent state fork가 아니라 loud connectivity failure로 나타난다.**

128비트를 쓴 이유도 명시돼 있다: 32비트 suffix는 grindable(2³² 해시로 fingerprint를 안 바꾸는 admitted
키를 찾아 silent fork를 되살릴 수 있음)이라서.

**wrong-dir guard `guard_join_descriptor` (`config.rs:293-309`)** — `join` 시 대상 dir에 이미 `network.toml`이
있고 `chain_id`가 다르면 거부한다. 같은 chain_id(문서화된 refreshed-invite 재조인)만 통과.
founder에게는 특히 치명적(재생성 불가한 chain_id를 남의 invite로 덮어쓰는 사고 방지).

---

## 3. Pre-genesis 라이프사이클 (`main.rs:744-970, 1230-1283`)

dispatch table (`main.rs:744-754`)이 config 해석 전에 verb를 가른다. founder와 joiner가 주고받는 순서:

| # | 실행자 | verb | 함수 | 소비 | 생성/변경 | stdout |
|---|--------|------|------|------|-----------|--------|
| 1 | Founder | `init --name --dir` | `cmd_init` `main.rs:833` | — (network.toml 있으면 실패) | `identity.key`, `network.toml`, `node.toml` | chain_id |
| 2 | Founder | `invite --config` | `cmd_invite` `main.rs:895` | descriptor+key | `network.toml`(자기 dial 힌트 추가) | **invite blob** |
| 3 | Joiner | `join <blob> --dir` | `cmd_join` `main.rs:1230` | blob | `network.toml`, fresh `identity.key`, `node.toml` | **joiner pubkey hex** |
| 4 | — | joiner가 pubkey를 founder에게 전달 (out-of-band) | | | | |
| 5 | Founder | `admit <pubkey>` | `cmd_admit` `main.rs:934` | descriptor | `network.toml`(validator 추가) | — |
| 6 | Founder | `invite` (재실행) | | | | **refreshed blob** |
| 7 | Joiner | `join <refreshed blob>` (같은 dir) | | refreshed blob | `network.toml` 덮어씀, identity **재사용** | "member 됨" |
| 8 | 양쪽 | `ducktape-node --config` | `run_node` | node.toml | `storage/` = **genesis FREEZE** | app-hash |

핵심 불변식:

- **`admit`은 founder 자기 파일만 편집** (`NetworkDescriptor::admit`, `config.rs:262`) → joiner 디스크엔 전파
  안 됨 → 그래서 joiner는 refreshed invite로 재조인해서 validator set을 일치시켜야 한다(§2의 fingerprint 때문에 필수).
- **FREEZE 지점 = 첫 `run_node`가 `storage/`를 만드는 순간.** `cmd_admit`은 `storage/` 존재 시 실행
  거부(`main.rs:952-959`, "running network는 governance(AddValidator)로 admit하지 genesis 편집으로 하지 않는다").
  즉 descriptor 편집은 storage 비어있을 때만 허용, 부팅 후 validator set은 immutable genesis floor가 되고
  이후 변경은 consensus-ordered op(§4)로만.
- **재사용 vs 재작성**: `identity.key`는 재조인해도 보존(`load_or_generate_identity`, `config.rs:69`),
  `network.toml`/`node.toml`은 매번 재작성. plumbing은 `merged_plumbing`(flags > 기존 파일 > defaults)으로
  partial-flag-safe.
- **`init`은 의도적으로 non-idempotent** — 재실행하면 새 chain_id를 mint해 기존 invite 소유자들을 silent하게
  "un-found"시키므로 막는다. 반면 `join`/`admit`은 idempotent.

---

## 4. Post-genesis admission — governance → cutover → promotion

genesis 후엔 파일 편집이 봉인되고, `invite-accept`가 **running 노드의 local RPC**를 통해 on-chain governance를 구동한다.

### (a) `cmd_invite_accept <pubkey>` (`main.rs:1076-1224`)

특수 네트워크 메시지가 아니라 얇은 CLI 드라이버:

1. 자기 identity로 서명 (`resolved.signer`), `rpc_listen` 필수.
2. precheck: 이미 member면 에러, 나 자신이 member 아니면 에러("only members admit validators").
3. **proposal 재사용/생성**: 같은 `AddValidator{key}`의 Open proposal이 있으면 `"joining open proposal {id}"`
   (`main.rs:1135`) — 여러 member가 동시에 같은 pubkey를 admit해도 중복 없이 같은 proposal로 수렴(idempotent).
   없으면 `admit:<hex>:<n>` id로 `GovMsg::Propose` (voting_period `1_000_000` = consensus-time상 far horizon,
   느린 2차 투표에도 만료 안 됨).
4. **투표**: 무조건 `Vote{approve:true}` — 제안자 포함 각 member가 자기 yes 던짐.
5. **execute-if-decidable**: `majority = members/2 + 1`. 미달이면 에러 아니라 `Ok(())`로 안내만(부분 quorum은
   정상 중간상태). 결정타를 던진 실행만 `GovMsg::Execute`.

governance 모듈(`crates/system/governance/src/lib.rs`)의 보증: `handle_propose/vote/execute`는 모두 **현재
valset member**(`Origin::External`, 암호학적으로 검증된 frame signer)만 허용, 재투표는 last-vote-wins,
execute 시 member 수를 **재조회**(제안 이후 membership 변동 반영). pass 시 같은 블록 follow-up으로
`ValsetMsg::Join{key}` emit — **governance가 valset membership 변경의 유일한 authorized 채널**.

### (b) Join → epoch cutover → mesh re-track

1. valset 모듈(`crates/system/valset/src/lib.rs`)의 `execute`가 origin을 `Module`/`System`으로 강제(External
   절대 불가) → `stage_add` → `commit_block`에서 `validators: BTreeSet`에 병합. `root()`는 정렬된 set의 sha256.
2. `node.watch_module("valset")`(`main.rs:2140`)로 모든 validator가 membership 변경을 **동일 view**에서 관측하도록 배치.
3. `ValsetOrchestrator::observe_members` (`valset_orchestrator.rs:209`)가 관측 set이 엔진의 현재 participant
   set과 다르면 `ScheduledCutover{cutover_view = finalized_view + CUTOVER_DELAY(=3)}` 무장. 여기서
   **epoch = 합의 엔진 incarnation 카운터** (app height와 별개, `app_height(view)=epoch_base+view`).
4. `respawn_if_due`가 boundary를 넘으면 `RespawnPlan` 반환 → **transport FIRST**:
   `mesh_oracle.track(epoch, mesh_at(members))` 먼저(`main.rs:2594`, 새 epoch mesh가 member를 admit해야
   그들에게 뭘 기대할 수 있음) → 그 다음 `spawn_epoch` + `node.cutover` → 즉시 checkpoint →
   `"cutover complete: epoch N …"` (테스트가 기다리는 마커).

> ⚠️ `crates/system/valset-mesh-interface`는 **아직 런타임에 배선 안 됨.** 이 crate는
> `(epoch, admission_root, validators) → MeshView`의 deterministic projection contract(모든 validator에게
> `validator_owned()` capability, content-addressed `MeshVersion`)를 정의하지만, 실제 cutover 경로는
> `main.rs`의 inline `mesh_at` 클로저가 commonware `discovery::Set`에 직접 track한다.
> **"future adapter contract"** — "interface가 cutover를 스케줄한다"고 쓰면 틀림. 스케줄러는 `ValsetOrchestrator`다.

### (c) Joiner 런타임 — park → statesync → self-promote (`main.rs:1308-1758`)

- **joiner-mode 감지**: `!sync_only && !validators.contains(self) && !promoted`
  (promoted = `recovery-manifest` 존재). fresh identity라 genesis set에 없음 → joiner.
- **parking** (`main.rs:1538-1667`): `"joiner mode: parking …"` 출력, genesis mesh를 base index에 track(권한은
  있지만 privilege 없는 peer로 mesh 합류). **consensus 엔진을 절대 안 만든다.** 대신 `EPOCH_CHANNEL_BANK(=16)`개
  epoch의 모든 consensus 채널을 등록하고 drain-and-discard(등록 안 된 채널은 protocol violation이라 연결이
  끊김). `P2pSyncClient`만 띄움.
- **poll loop**: 2초마다 `fetch_manifest`. manifest는 `participants`(serving validator의 현재 engine set)를 담음.
  **admission 판정 = 내 키가 `m.participants`에 있는지**(bare count 아님). 없으면
  `"parked: awaiting admission (epoch N has M validators)"` 출력하고 계속. 중간 epoch cutover도 따라가며 mesh track 갱신.
- **self-promote**: 내 키가 participants에 들어오면 → `floor_cert`를 epoch scheme로 **독립 검증**("거짓말하는
  source는 여기서 fail, validator 부팅 후 brick 아님") → `sync_all_modules` → recovery checkpoint를
  **위조(fabricate)**해서 로컬 recovery store에 기록 → `"promoted: validator at epoch N …"` →
  **`reboot_self()`로 프로세스 재실행**. 재부팅 시 `promoted` probe가 참이 되어 normal validator/recovery
  경로로 부팅, 이제 실제로 투표. **promotion은 in-process 상태 전이가 아니라 re-exec다.**

---

## 5. Desktop UX 래핑 (`app/src-tauri/src/workspaces.rs` + React)

데스크톱 앱은 **invite/crypto 로직을 재구현하지 않는다** — `ducktape-node` 바이너리를 subprocess로 부르는
얇은 orchestration+registry 계층(`run_verb`, `workspaces.rs:199`).

- Tauri commands: `workspace_create`(→`init`+`keygen`), `workspace_join`(→`join`, stdout=joiner pubkey),
  `workspace_invite_blob`(→`invite`), `workspace_admit`(→`invite-accept`), `workspace_select`(노드 detach spawn),
  `workspace_phase`.
- **parked joiner는 HTTP/RPC surface가 없으므로** 진행상황을 `daemon.log` tail의 마커
  문자열(`"joiner mode: parking"`/`"parked:"`/`"admitted at epoch"`/`"synced app_hash="`/`"promoted:"`/`"FATAL"`)로
  `starting|parked|admitted|synced|promoted|fatal` phase에 매핑(`classify`, `workspaces.rs:571`).
  CLI 마커가 곧 UX API인 셈.
- **advertised addr 우선순위** (`advertised_addr`, `workspaces.rs:305`): ① `DUCKTAPE_ADVERTISE_ADDR`(full
  host:port verbatim, 리버스 프록시 뒤 도메인) → ② `DUCKTAPE_ADVERTISE_HOST` + local port → ③ `api.ipify.org`로
  public IP 자동 발견(TLS 없는 raw TCP, 4s best-effort) → ④ `127.0.0.1:port`. mesh listen은 항상 `0.0.0.0:port`,
  advertised만 dialable 주소로 `--advertised` 플래그에 전달 → 이게 blob에 박힌다.
- **blob 복붙 UX**: 생성측 `SettingsView`의 read-only textarea(click하면 auto-select, 별도 Copy 버튼 없음),
  조인측 `OnboardingGate`의 `"Paste invite blob (ducktape-invite-v2:…)"` textarea. 역방향(joiner pubkey→member)은
  `JoinProgress`에서 `navigator.clipboard.writeText`. 프론트/Tauri는 blob 내부를 **파싱하지 않음** —
  trim + non-empty 체크만 하는 passthrough.

---

## 6. 알려진 이슈 / 갭

1. 🐛 **busy-source admission fork (미해결, in-flight)** — 워킹트리에 uncommitted 상태.
   `HEARTBEAT_INTERVAL=1s`의 idle nop(`main.rs:2667`)이 finalized view를 계속 틱하게 해 cutover가 무트래픽에도
   넘어가게 하지만, 이 "never-quiescing source"가 admission 중 **JOINING validator의 state를 fork**시킨다.
   fault line은 resolver-lane 모듈(`kv`/`document`/`chat`)의 **live qmdb target을 frozen manifest root와
   post-hoc 대조**하는 부분(`main.rs:444,458` "live target moved past the captured boundary (busy source)").
   snapshot-lane(directory/valset/saga/governance/tasks)은 height-pinned라 안전.
   `handoff-blocktime-fork.md`에 "fix 랜딩+검증 전엔 heartbeat/pacing 절대 ship 금지" 명시.
2. **테스트 커버리지 갭** — pre-genesis `admit` verb를 end-to-end로 구동하는 Rust e2e가 **없다.**
   `invite_e2e.rs`/`live_admission_e2e.rs`는 둘 다 post-genesis `invite-accept`(governance) 경로만 검증.
   pre-genesis는 `demo-invite.sh`(shell)와 `config.rs` 단위테스트로만 커버.
3. **valset-mesh-interface 미배선** (§4 참고) — 존재하지만 hot path 밖의 미래 계약.

---

## 부록: 파일 인덱스

| 계층 | 파일 | 핵심 심볼 |
|------|------|-----------|
| Wire format | `bin/node/src/config.rs:363-510` | `pack_invite` / `unpack_invite` / `encode_invite` / `decode_invite` / `InviteReader` |
| Descriptor | `bin/node/src/config.rs:136-278` | `NetworkDescriptor` / `mint_chain_id` / `genesis_namespace` / `guard_join_descriptor` / `dialable` |
| CLI verbs | `bin/node/src/main.rs:744-970` | `cmd_keygen` / `cmd_init` / `cmd_invite` / `cmd_admit` |
| Join + joiner 런타임 | `bin/node/src/main.rs:1230-1758` | `cmd_join` / `cmd_invite_accept` / joiner park·sync·promote |
| Epoch cutover | `crates/kernel/consensus/src/valset_orchestrator.rs` | `observe_members` / `respawn_if_due` / `ScheduledCutover` / `RespawnPlan` |
| Governance | `crates/system/governance/src/lib.rs:200-328` | `handle_propose` / `handle_vote` / `handle_execute` |
| Valset | `crates/system/valset/src/lib.rs:233-276` | `execute` origin gate / `Join` / `commit_block` |
| Mesh 계약(미배선) | `crates/system/valset-mesh-interface/src/lib.rs` | `derive_mesh` / `MeshView` / `MeshVersion` |
| Desktop 래퍼 | `app/src-tauri/src/workspaces.rs` | `workspace_create/join/invite_blob/admit` / `advertised_addr` / `classify` |
| Desktop UI | `app/src/console/views/onboarding/{OnboardingGate,JoinProgress}.tsx`, `views/settings/SettingsView.tsx` | invite 생성·복붙·admit UI |
| 테스트 | `bin/node/tests/{invite_e2e,live_admission_e2e}.rs`, `bin/node/examples/demo-invite.sh` | admission 마커 시퀀스 |
