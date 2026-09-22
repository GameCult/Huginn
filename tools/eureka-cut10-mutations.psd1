# Cut 10 mutations, D1-D22: the CultNet surface's rules. Each rule has a
# revert, `Dn`, which removes the rule, and a loosening, `DnL`, which weakens
# it rather than removing it, because a plain revert is the easy target. Every
# entry names the test that must fail while it is applied. Run through
# Epiphany's harness from this repo:
#
#   $env:CARGO_TARGET_DIR = 'C:\Users\Meta\.cargo-target-codex'
#   powershell -File F:\Projects\Epiphany\tools\eureka-mutations.ps1 -Repo F:\Projects\Huginn `
#       -Entries tools/eureka-cut10-mutations.psd1 `
#       -Target crates/huginn-mind/src/mind.rs,crates/huginn-mind/src/wire.rs,crates/huginn-daemon/src/daemon.rs,crates/huginn-daemon/src/envelope.rs,crates/huginn-daemon/src/serve.rs,schemas/cultnet/huginn.mind_request.v1.schema.json,schemas/cultnet/huginn.mind_response.v1.schema.json `
#       -Test 'cargo test -p huginn-daemon --lib'
#
# Entries whose test lives in `huginn-mind` carry their own `Command`. `D1` and
# `D1D` are one mutant run against two suites, which is the point of the rule:
# `Mind::require_instance` is the one instance check, and admission and the
# daemon's reads are its two callers. The same edit is `H1` in
# `eureka-cut8-mutations.psd1`.
#
# `M0` is the harness's own no-op control and is not listed here.
#
# Two rules have no loosening and say so rather than carrying a weak entry:
#
# - `D9`, the connection id the hub serves. It is one `u32` handed to
#   `CultNetRudpServerHubOptions::new`; a hub serves that id or another one,
#   and there is no shape of "serves more ids" to weaken it into.
# - `D12`'s order, weakened as far as it goes by `D12L`: a hub bound before the
#   mind is opened and then dropped when the mind refuses. Any weaker form is
#   the correct order again.
#
# `D17`-`D19` pin three of the dispatch's four read arms and `D23` the fourth.
# The fourth was once recorded here as unpinnable, on the grounds that
# `Mind::open_items` raises no refusal of its own; the refusal it carries comes
# from the reader beneath it over a store whose receipt is gone, and such a
# store is reachable from this crate's surface, so the entry exists.
@{
    Mutations = @(
        @{
            Id      = 'D1'
            Rule    = 'Ruling 14: the mind owns the instance check, and it is a comparison.'
            Test    = 'mind::tests::require_instance_is_the_one_check_admission_and_the_daemon_share'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/mind.rs'
            Old     = @'
        if declared != self.instance() {
            return Err(MindRefusal::ForeignInstance {
                declared: declared.0.clone(),
                mind: self.instance().0.clone(),
            });
        }
        Ok(())
'@
            New     = @'
        let _ = declared;
        Ok(())
'@
        }
        @{
            Id      = 'D1L'
            Rule    = 'The instance check compares the names, not their lengths.'
            Test    = 'mind::tests::require_instance_is_the_one_check_admission_and_the_daemon_share'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/mind.rs'
            Old     = '        if declared != self.instance() {'
            New     = '        if declared.0.len() != self.instance().0.len() {'
        }
        @{
            Id      = 'D1L2'
            Rule    = 'It compares the names whole, not their first byte.'
            Test    = 'mind::tests::require_instance_is_the_one_check_admission_and_the_daemon_share'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/mind.rs'
            Old     = '        if declared != self.instance() {'
            New     = '        if declared.0.as_bytes().first() != self.instance().0.as_bytes().first() {'
        }
        @{
            Id      = 'D1L3'
            Rule    = 'It runs on every declared name, not only on one long enough to look suspicious.'
            Test    = 'mind::tests::require_instance_is_the_one_check_admission_and_the_daemon_share'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/mind.rs'
            Old     = '        if declared != self.instance() {'
            New     = '        if declared.0.len() > self.instance().0.len() && declared != self.instance() {'
        }
        @{
            Id      = 'D1L4'
            Rule    = 'It compares the names, not whether one begins with the other.'
            Test    = 'mind::tests::require_instance_is_the_one_check_admission_and_the_daemon_share'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/mind.rs'
            Old     = '        if declared != self.instance() {'
            New     = '        if !declared.0.starts_with(&self.instance().0) {'
        }
        @{
            Id      = 'D1L5'
            Rule    = 'It compares the names as written: a slug may carry uppercase, so two names differing in case are two minds.'
            Test    = 'mind::tests::require_instance_is_the_one_check_admission_and_the_daemon_share'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/mind.rs'
            Old     = '        if declared != self.instance() {'
            New     = '        if !declared.0.eq_ignore_ascii_case(&self.instance().0) {'
        }
        @{
            Id      = 'D1N'
            Rule    = 'The refusal names which mind refused, so an agent can act on the field rather than guess.'
            Test    = 'mind::tests::require_instance_is_the_one_check_admission_and_the_daemon_share'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/mind.rs'
            Old     = '                mind: self.instance().0.clone(),'
            New     = '                mind: declared.0.clone(),'
        }
        @{
            Id   = 'D1D'
            Rule = 'One check, two callers: the daemon''s reads are refused by the same function admission asks.'
            Test = 'daemon::tests::a_read_or_a_write_naming_another_instance_is_refused_by_the_mind_and_writes_nothing'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = @'
        if declared != self.instance() {
            return Err(MindRefusal::ForeignInstance {
                declared: declared.0.clone(),
                mind: self.instance().0.clone(),
            });
        }
        Ok(())
'@
            New  = @'
        let _ = declared;
        Ok(())
'@
        }
        @{
            Id   = 'D1DL'
            Rule = 'The same comparison, reached the daemon''s way: a read naming a name of the mind''s own length is refused too.'
            Test = 'daemon::tests::a_read_or_a_write_naming_another_instance_is_refused_by_the_mind_and_writes_nothing'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = '        if declared != self.instance() {'
            New  = '        if declared.0.len() != self.instance().0.len() {'
        }
        @{
            Id   = 'D1DL2'
            Rule = 'And case, the daemon''s way: the daemon''s own fixtures carry a name differing from the mind''s in case alone.'
            Test = 'daemon::tests::a_read_or_a_write_naming_another_instance_is_refused_by_the_mind_and_writes_nothing'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = '        if declared != self.instance() {'
            New  = '        if !declared.0.eq_ignore_ascii_case(&self.instance().0) {'
        }
        @{
            Id      = 'S3wide'
            Rule    = 'The grammar check runs on the declared bytes as given: nothing folds a fullwidth character to its plain ASCII form before asking the leaf whether the name is grammatical.'
            Test    = 'daemon::tests::a_declared_instance_outside_the_grammar_is_refused_by_name_before_the_identity_check'
            Command = 'cargo test -p huginn-daemon --lib'
            File    = 'crates/huginn-mind/src/mind.rs'
            Old     = @'
pub fn require_grammatical_instance(declared: &Slug) -> Result<(), MindRefusal> {
    let probe = PipelineDocument::Instance(PipelineInstance {
        instance: declared.clone(),
        display_name: Short("grammar-probe".into()),
        created_at: Date("2026-01-01".into()),
        host: Short("grammar-probe".into()),
    });
    probe.validate().map_err(MindRefusal::Document)
}
'@
            New     = @'
pub fn require_grammatical_instance(declared: &Slug) -> Result<(), MindRefusal> {
    let folded: String = declared
        .0
        .chars()
        .map(|c| match c as u32 {
            0xFF01..=0xFF5E => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect();
    let probe = PipelineDocument::Instance(PipelineInstance {
        instance: Slug(folded),
        display_name: Short("grammar-probe".into()),
        created_at: Date("2026-01-01".into()),
        host: Short("grammar-probe".into()),
    });
    probe.validate().map_err(MindRefusal::Document)
}
'@
        }
        @{
            Id   = 'D2'
            Rule = 'Rulings 14 and 18 on the wire: the daemon passes the declared instance to the mind as the client sent it.'
            Test = 'daemon::tests::a_read_or_a_write_naming_another_instance_is_refused_by_the_mind_and_writes_nothing'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
            HuginnMindRequest::Admit(batch) => {
                let outcome = self.mind.admit(batch, now);
'@
            New  = @'
            HuginnMindRequest::Admit(mut batch) => {
                batch.instance = self.mind.instance().clone();
                let outcome = self.mind.admit(batch, now);
'@
        }
        @{
            Id   = 'D2L'
            Rule = 'Every read that names an instance is checked, not only the one that returns a page.'
            Test = 'daemon::tests::a_read_or_a_write_naming_another_instance_is_refused_by_the_mind_and_writes_nothing'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = '            && !matches!(request, HuginnMindRequest::Admit(_))'
            New  = '            && matches!(request, HuginnMindRequest::Query { .. })'
        }
        @{
            Id   = 'D3'
            Rule = 'A refusal is data in the response, never a transport failure the client must parse out of a failure envelope.'
            Test = 'serve::tests::a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '                let reply = encode_or_fail(&message_id, operation, &response, &runtime_id);'
            New  = @'
                if let HuginnMindResponse::Refused(refusal) = &response {
                    return encode_failure(
                        &message_id,
                        operation,
                        &crate::envelope::OperationFailure {
                            code: "refused".into(),
                            message: format!("{refusal}"),
                        },
                        &runtime_id,
                    );
                }
                let reply = encode_or_fail(&message_id, operation, &response, &runtime_id);
'@
        }
        @{
            Id      = 'D3L'
            Rule    = 'The envelope''s status is derived from the response, so a refusal cannot go out as an acceptance.'
            Test    = 'wire::tests::the_wire_vocabulary_is_the_minds_methods_and_status_is_derived_from_the_response'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/wire.rs'
            Old     = '            Self::Refused(_) | Self::Admit(PipelineAdmissionOutcome::Refused(_)) => "rejected",'
            New     = '            Self::Admit(PipelineAdmissionOutcome::Refused(_)) => "rejected",'
        }
        @{
            Id   = 'D4'
            Rule = 'An envelope the daemon cannot decode touches no mind; it is answered with its own failure code.'
            Test = 'serve::tests::a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind'
            File = 'crates/huginn-daemon/src/envelope.rs'
            Old  = @'
    let request: HuginnMindRequest = rmp_serde::from_slice(&bytes)
        .map_err(|error| OperationFailure::new("payload-not-a-request", error.to_string()))?;
'@
            New  = @'
    let request: HuginnMindRequest = rmp_serde::from_slice(&bytes).unwrap_or(HuginnMindRequest::Whoami);
'@
        }
        @{
            Id   = 'D4L'
            Rule = 'The envelope is addressed to this service, and a request for another one is refused rather than served.'
            Test = 'serve::tests::a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind'
            File = 'crates/huginn-daemon/src/envelope.rs'
            Old  = @'
    if service_id != MIND_SERVICE_ID {
        return Err(OperationFailure::new("wrong-service", format!("{service_id} is not {MIND_SERVICE_ID}")));
    }
'@
            New  = @'
    let _ = service_id;
'@
        }
        @{
            Id   = 'D5'
            Rule = 'The envelope''s operation string is checked against the decoded payload, never trusted.'
            Test = 'serve::tests::a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind'
            File = 'crates/huginn-daemon/src/envelope.rs'
            Old  = @'
    if operation != request.operation() {
        return Err(OperationFailure::new(
            "operation-mismatch",
            format!("the envelope says {operation} and the payload is {}", request.operation()),
        ));
    }
'@
            New  = @'
    let _ = operation;
'@
        }
        @{
            Id   = 'D5L'
            Rule = 'The operation is checked as the name it is, not as a shape that happens to match.'
            Test = 'serve::tests::a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind'
            File = 'crates/huginn-daemon/src/envelope.rs'
            Old  = '    if operation != request.operation() {'
            New  = '    if operation.len() != request.operation().len() {'
        }
        @{
            Id   = 'D20'
            Rule = 'An answer larger than one send can carry is refused by name, not handed to a send that fails silently.'
            Test = 'serve::tests::an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '    if encoded <= MAX_RESPONSE_BYTES {'
            New  = '    if true {'
        }
        @{
            Id   = 'D20L'
            Rule = 'It is measured against the window the hub was configured with, not against a number chosen to admit it.'
            Test = 'serve::tests::an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '    if encoded <= MAX_RESPONSE_BYTES {'
            New  = '    if encoded <= MAX_RESPONSE_BYTES * 4 {'
        }
        @{
            Id   = 'D20L2'
            Rule = 'The refusal carries the answer''s own encoded size, so a caller can tell how far it overshot.'
            Test = 'serve::tests::an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '        bytes: encoded,'
            New  = '        bytes: MAX_RESPONSE_BYTES,'
        }
        @{
            Id   = 'D20L3'
            Rule = 'And it carries the limit itself, not the size again, so the two are not one number twice.'
            Test = 'serve::tests::an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '        limit: MAX_RESPONSE_BYTES,'
            New  = '        limit: encoded,'
        }
        @{
            Id   = 'D20L4'
            Rule = 'The limit itself is deliverable: the comparison admits an answer of exactly the window''s size, as the transport does.'
            Test = 'serve::tests::the_gates_boundary_is_the_transports_boundary_byte_for_byte'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '    if encoded <= MAX_RESPONSE_BYTES {'
            New  = '    if encoded < MAX_RESPONSE_BYTES {'
        }
        @{
            Id   = 'D20L5'
            Rule = 'What is measured is the encoded envelope the hub fragments, not the payload string inside it.'
            Test = 'serve::tests::the_gates_boundary_is_the_transports_boundary_byte_for_byte'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = @'
    let encoded = match encode_cultnet_message_to_vec(&reply, CultNetWireContract::CultNetSchemaV0) {
        Ok(bytes) => bytes.len() as u64,
'@
            New  = @'
    let encoded = match encode_cultnet_message_to_vec(&reply, CultNetWireContract::CultNetSchemaV0) {
        Ok(bytes) => match &reply {
            CultNetMessage::OperationResponse { payload, .. } => payload.len() as u64,
            _ => bytes.len() as u64,
        },
'@
        }
        @{
            Id   = 'D21'
            Rule = 'The refusal rides the response schema like every other refusal, not a failure envelope the client must parse apart.'
            Test = 'serve::tests::an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = @'
    let refusal = HuginnMindResponse::Refused(MindRefusal::ResponseTooLarge {
        bytes: encoded,
        limit: MAX_RESPONSE_BYTES,
    });
    encode_or_fail(message_id, operation, &refusal, runtime_id)
'@
            New  = @'
    encode_failure(
        message_id,
        operation,
        &crate::envelope::OperationFailure {
            code: "response-too-large".into(),
            message: format!("{encoded} bytes against a {MAX_RESPONSE_BYTES} limit"),
        },
        runtime_id,
    )
'@
        }
        @{
            Id   = 'D21L'
            Rule = 'It echoes the client''s own correlation key, so a refusal can be matched to the request that earned it.'
            Test = 'serve::tests::an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '    encode_or_fail(message_id, operation, &refusal, runtime_id)'
            New  = '    encode_or_fail("response-too-large", operation, &refusal, runtime_id)'
        }
        @{
            Id   = 'D22'
            Rule = 'The window the gate measures against is the window the hub carries: a smaller one sends nothing and says nothing.'
            Test = 'serve::tests::an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '    options.max_pending_reliable_packets = Some(MAX_PENDING_RELIABLE_PACKETS);'
            New  = '    options.max_pending_reliable_packets = Some(MAX_PENDING_RELIABLE_PACKETS / 2);'
        }
        @{
            Id   = 'D22L'
            Rule = 'Not merely close to it: a window short by a few dozen packets still drops an answer the gate admitted.'
            Test = 'serve::tests::an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '    options.max_pending_reliable_packets = Some(MAX_PENDING_RELIABLE_PACKETS);'
            New  = '    options.max_pending_reliable_packets = Some(MAX_PENDING_RELIABLE_PACKETS - 64);'
        }
        @{
            Id   = 'D22L2'
            Rule = 'The fragment size is the window''s other half: a smaller one carries fewer bytes in the same packet count.'
            Test = 'serve::tests::an_answer_too_large_for_one_send_is_a_typed_refusal_that_reaches_the_client'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '    options.max_fragment_bytes = Some(MAX_FRAGMENT_BYTES);'
            New  = '    options.max_fragment_bytes = Some(MAX_FRAGMENT_BYTES - 64);'
        }
        @{
            Id   = 'D17'
            Rule = 'A refusal `view` raised crosses the dispatch as itself, not rewrapped as unavailable.'
            Test = 'daemon::tests::a_refusal_a_read_raised_is_the_answer_the_dispatch_returns_whole'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
            HuginnMindRequest::View { id, .. } => match self.mind.view(&id) {
                Ok(view) => HuginnMindResponse::View(view),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
'@
            New  = @'
            HuginnMindRequest::View { id, .. } => match self.mind.view(&id) {
                Ok(view) => HuginnMindResponse::View(view),
                Err(refusal) => HuginnMindResponse::Refused(huginn_mind::MindRefusal::Unavailable {
                    detail: format!("{refusal}"),
                }),
            },
'@
        }
        @{
            Id   = 'D17L'
            Rule = 'It is not swallowed into an empty answer either: a malformed reference is not an absent document.'
            Test = 'daemon::tests::a_refusal_a_read_raised_is_the_answer_the_dispatch_returns_whole'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
            HuginnMindRequest::Query { query, .. } => match self.mind.query(&query) {
'@
            New  = @'
                Err(_) => HuginnMindResponse::View(None),
            },
            HuginnMindRequest::Query { query, .. } => match self.mind.query(&query) {
'@
        }
        @{
            Id   = 'D18'
            Rule = 'A refusal `query` raised crosses the dispatch as itself, with the mind''s own detail.'
            Test = 'daemon::tests::a_refusal_a_read_raised_is_the_answer_the_dispatch_returns_whole'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
            HuginnMindRequest::Query { query, .. } => match self.mind.query(&query) {
                Ok(page) => HuginnMindResponse::Query(page),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
'@
            New  = @'
            HuginnMindRequest::Query { query, .. } => match self.mind.query(&query) {
                Ok(page) => HuginnMindResponse::Query(page),
                Err(refusal) => HuginnMindResponse::Refused(huginn_mind::MindRefusal::Unavailable {
                    detail: format!("a query was refused: {refusal}"),
                }),
            },
'@
        }
        @{
            Id   = 'D18L'
            Rule = 'Nor is it flattened into an empty page, which would read as a mind holding nothing.'
            Test = 'daemon::tests::a_refusal_a_read_raised_is_the_answer_the_dispatch_returns_whole'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
                Ok(page) => HuginnMindResponse::Query(page),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
'@
            New  = @'
                Ok(page) => HuginnMindResponse::Query(page),
                Err(_) => HuginnMindResponse::Query(huginn_mind::PipelineQueryPage { items: vec![], matched: 0 }),
'@
        }
        @{
            Id   = 'D19'
            Rule = 'A refusal `history` raised crosses the dispatch as itself, not as another read''s refusal.'
            Test = 'daemon::tests::a_refusal_a_read_raised_is_the_answer_the_dispatch_returns_whole'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
            HuginnMindRequest::History { scope, .. } => match self.mind.history(&scope) {
                Ok(views) => HuginnMindResponse::History(views),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
'@
            New  = @'
            HuginnMindRequest::History { scope, .. } => match self.mind.history(&scope) {
                Ok(views) => HuginnMindResponse::History(views),
                Err(_) => HuginnMindResponse::Refused(huginn_mind::MindRefusal::Unavailable {
                    detail: "semantic query: the index is not wired (Cut 11)".into(),
                }),
            },
'@
        }
        @{
            Id   = 'D19L'
            Rule = 'Nor is a history refusal swallowed into an empty history, which is the answer for a subject with no records.'
            Test = 'daemon::tests::a_refusal_a_read_raised_is_the_answer_the_dispatch_returns_whole'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
                Ok(views) => HuginnMindResponse::History(views),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
'@
            New  = @'
                Ok(views) => HuginnMindResponse::History(views),
                Err(_) => HuginnMindResponse::History(vec![]),
'@
        }
        @{
            Id   = 'D23'
            Rule = 'A refusal `open_items` carries out of the reader crosses the dispatch as itself, not rewrapped as unavailable.'
            Test = 'daemon::tests::a_refusal_open_items_raised_is_the_answer_the_dispatch_returns_whole'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
            HuginnMindRequest::OpenItems { campaign, .. } => match self.mind.open_items(&campaign) {
                Ok(items) => HuginnMindResponse::OpenItems(items),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
            },
'@
            New  = @'
            HuginnMindRequest::OpenItems { campaign, .. } => match self.mind.open_items(&campaign) {
                Ok(items) => HuginnMindResponse::OpenItems(items),
                Err(refusal) => HuginnMindResponse::Refused(huginn_mind::MindRefusal::Unavailable {
                    detail: format!("{refusal}"),
                }),
            },
'@
        }
        @{
            Id   = 'D23L'
            Rule = 'Nor is it swallowed into an empty open set, which is what a healthy campaign with nothing open answers.'
            Test = 'daemon::tests::a_refusal_open_items_raised_is_the_answer_the_dispatch_returns_whole'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
                Ok(items) => HuginnMindResponse::OpenItems(items),
                Err(refusal) => HuginnMindResponse::Refused(refusal),
'@
            New  = @'
                Ok(items) => HuginnMindResponse::OpenItems(items),
                Err(_) => HuginnMindResponse::OpenItems(huginn_mind::PipelineOpenItems {
                    questions: vec![],
                    findings: vec![],
                    follow_ups: vec![],
                    specs_without_report: vec![],
                    reports_without_verdict: vec![],
                }),
'@
        }
        @{
            Id   = 'D5L2'
            Rule = 'The operation is compared as bytes: another casing of the name is another name.'
            Test = 'serve::tests::a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind'
            File = 'crates/huginn-daemon/src/envelope.rs'
            Old  = '    if operation != request.operation() {'
            New  = '    if !operation.eq_ignore_ascii_case(request.operation()) {'
        }
        @{
            Id   = 'D6'
            Rule = 'The index never decides an outcome: a sink that fails does not refuse a batch the mind admitted.'
            Test = 'daemon::tests::the_index_is_handed_every_landed_write_after_commit_and_never_decides_the_outcome'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
                    let refs = writes.iter().map(|write| write.id.0.clone()).collect::<Vec<_>>().join(", ");
                    eprintln!("huginn: the index refused the landed writes [{refs}]: {error:#}");
'@
            New  = @'
                    return HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(
                        huginn_mind::MindRefusal::Unavailable { detail: format!("{error:#}") },
                    ));
'@
        }
        @{
            Id   = 'D6L'
            Rule = 'The index does not change the answer either: the outcome is the mind''s, whatever the sink did with it.'
            Test = 'daemon::tests::the_index_is_handed_every_landed_write_after_commit_and_never_decides_the_outcome'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = @'
                    let refs = writes.iter().map(|write| write.id.0.clone()).collect::<Vec<_>>().join(", ");
                    eprintln!("huginn: the index refused the landed writes [{refs}]: {error:#}");
'@
            New  = @'
                    return HuginnMindResponse::Admit(PipelineAdmissionOutcome::AlreadyAdmitted {
                        receipt_id: format!("index-unavailable: {error:#}"),
                    });
'@
        }
        @{
            Id   = 'D7'
            Rule = 'The index is handed every landed write.'
            Test = 'daemon::tests::the_index_is_handed_every_landed_write_after_commit_and_never_decides_the_outcome'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = '                    && let Err(error) = self.index.committed(&self.mind, writes)'
            New  = '                    && let Err(error) = self.index.committed(&self.mind, &[])'
        }
        @{
            Id   = 'D7L'
            Rule = 'Derived writes are landed writes: a ruling that answers a question hands the index two documents, not one.'
            Test = 'daemon::tests::the_index_is_handed_every_landed_write_after_commit_and_never_decides_the_outcome'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = '                    && let Err(error) = self.index.committed(&self.mind, writes)'
            New  = '                    && let Err(error) = self.index.committed(&self.mind, &writes[..1])'
        }
        @{
            Id   = 'D8'
            Rule = 'Each reply goes to the session its request arrived on.'
            Test = 'serve::tests::two_clients_get_their_own_replies_over_loopback'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = @'
            let reply = answer(daemon, registry, message, Utc::now());
            if let Err(error) = hub.send_schema_message(&session, &reply) {
'@
            New  = @'
            let reply = answer(daemon, registry, message, Utc::now());
            let session = hub.sessions().into_iter().next().unwrap_or(session);
            if let Err(error) = hub.send_schema_message(&session, &reply) {
'@
        }
        @{
            Id   = 'D8L'
            Rule = 'A reply goes to one session, not to everyone connected: another peer''s answer is not this peer''s to read.'
            Test = 'serve::tests::two_clients_get_their_own_replies_over_loopback'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = @'
            if let Err(error) = hub.send_schema_message(&session, &reply) {
                eprintln!("huginn: {} did not receive its reply: {error:#}", session.remote_addr);
            }
'@
            New  = @'
            for session in hub.sessions() {
                if let Err(error) = hub.send_schema_message(&session, &reply) {
                    eprintln!("huginn: {} did not receive its reply: {error:#}", session.remote_addr);
                }
            }
'@
        }
        @{
            Id   = 'D9'
            Rule = 'The hub serves CultNet''s operation connection id and no other, so a peer speaking another protocol on the port never becomes a session.'
            Test = 'serve::tests::two_clients_get_their_own_replies_over_loopback'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = '    let mut options = CultNetRudpServerHubOptions::new(runtime_id, socket, CULTNET_OPERATION_CONNECTION_ID);'
            New  = '    let mut options = CultNetRudpServerHubOptions::new(runtime_id, socket, 0x4355_4c55);'
        }
        @{
            Id   = 'D10'
            Rule = '`whoami` is derived from the mind: `documents` counts pipeline documents, not every row the store holds.'
            Test = 'daemon::tests::a_batch_round_trips_typed_through_handle_without_a_socket'
            File = 'crates/huginn-mind/src/wire.rs'
            Old  = @'
            } else if PipelineKind::ALL.iter().any(|kind| kind.type_id() == envelope.r#type) {
                documents += 1;
            }
'@
            New  = @'
            } else {
                documents += 1;
            }
'@
        }
        @{
            Id   = 'D10L'
            Rule = 'A mind says how many receipts it holds, so a reader can tell a mind that has admitted from one that has not.'
            Test = 'daemon::tests::a_batch_round_trips_typed_through_handle_without_a_socket'
            File = 'crates/huginn-mind/src/wire.rs'
            Old  = @'
            if envelope.r#type == HuginnCommitReceipt::TYPE {
                receipts += 1;
'@
            New  = @'
            if envelope.r#type == HuginnCommitReceipt::TYPE {
                receipts += 0;
'@
        }
        @{
            Id   = 'D11'
            Rule = 'Ruling 15: a mind that will not open is refused, and no other mind is opened in its place.'
            Test = 'daemon::tests::the_daemon_refuses_loudly_when_it_cannot_open_the_mind_and_binds_nothing'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = '        Ok(Self { mind: Mind::open(state_root, instance)?, index: NoIndex })'
            New  = @'
        let mind = match Mind::open(state_root, instance) {
            Ok(mind) => mind,
            Err(_) => Mind::open(&state_root.join("fallback"), instance)?,
        };
        Ok(Self { mind, index: NoIndex })
'@
        }
        @{
            Id   = 'D11L'
            Rule = 'Every refusal of the opener is a refusal to serve, not only the held lock: a store naming another instance is not this daemon''s to open.'
            Test = 'daemon::tests::the_daemon_refuses_loudly_when_it_cannot_open_the_mind_and_binds_nothing'
            File = 'crates/huginn-daemon/src/daemon.rs'
            Old  = '        Ok(Self { mind: Mind::open(state_root, instance)?, index: NoIndex })'
            New  = @'
        let mind = match Mind::open(state_root, instance) {
            Ok(mind) => mind,
            Err(held @ MindRefusal::MindAlreadyOwned { .. }) => return Err(held),
            Err(_) => Mind::open(&state_root.join("fallback"), instance)?,
        };
        Ok(Self { mind, index: NoIndex })
'@
        }
        @{
            Id   = 'D12'
            Rule = 'Ruling 15 as an order: the mind opens before anything binds, so a refused mind is reported as a refused mind and no socket is ever taken.'
            Test = 'serve::tests::startup_opens_the_mind_before_it_binds_anything'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = @'
    let daemon = Daemon::open(&options.state_root, &options.instance)?;
    let hub = bind(options.bind, &daemon.runtime_id())?;
'@
            New  = @'
    let hub = bind(options.bind, "huginn")?;
    let daemon = Daemon::open(&options.state_root, &options.instance)?;
'@
        }
        @{
            Id   = 'D12L'
            Rule = 'Tidying up afterwards is not the order: a socket taken before the mind opened was still taken, and the refusal still comes back as the port''s rather than the mind''s.'
            Test = 'serve::tests::startup_opens_the_mind_before_it_binds_anything'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = @'
    let daemon = Daemon::open(&options.state_root, &options.instance)?;
    let hub = bind(options.bind, &daemon.runtime_id())?;
'@
            New  = @'
    let hub = bind(options.bind, "huginn")?;
    let daemon = match Daemon::open(&options.state_root, &options.instance) {
        Ok(daemon) => daemon,
        Err(refusal) => {
            drop(hub);
            return Err(refusal.into());
        }
    };
    let hub = bind(options.bind, &daemon.runtime_id())?;
'@
        }
        @{
            Id      = 'D13'
            Rule    = 'The wire vocabulary is the mind''s methods: one operation name per method, and it is that method''s name.'
            Test    = 'wire::tests::the_wire_vocabulary_is_the_minds_methods_and_status_is_derived_from_the_response'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/wire.rs'
            Old     = '            Self::History { .. } => "history",'
            New     = '            Self::History { .. } => "query",'
        }
        @{
            Id      = 'D13L'
            Rule    = 'The operation name is the method''s spelling, so a client reading `Mind`''s surface can address it without a translation table.'
            Test    = 'wire::tests::the_wire_vocabulary_is_the_minds_methods_and_status_is_derived_from_the_response'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'crates/huginn-mind/src/wire.rs'
            Old     = '            Self::OpenItems { .. } => "open_items",'
            New     = '            Self::OpenItems { .. } => "openItems",'
        }
        @{
            Id      = 'D14'
            Rule    = 'The published request schema equals its derivation, byte for byte.'
            Test    = 'wire::tests::published_wire_schemas_match_derivation'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'schemas/cultnet/huginn.mind_request.v1.schema.json'
            Old     = '  "title": "HuginnMindRequest",'
            New     = '   "title": "HuginnMindRequest",'
        }
        @{
            Id      = 'D14L'
            Rule    = 'Both files are pinned, not one: the response schema is derived too, and a hand edit to it is the same drift.'
            Test    = 'wire::tests::published_wire_schemas_match_derivation'
            Command = 'cargo test -p huginn-mind --lib'
            File    = 'schemas/cultnet/huginn.mind_response.v1.schema.json'
            Old     = '  "title": "HuginnMindResponse",'
            New     = '  "title": "HuginnMindResponse v1",'
        }
        @{
            Id   = 'D15'
            Rule = 'A replay is an answer, not a rejection: `AlreadyAdmitted` tells a client its batch is in the mind already.'
            Test = 'envelope::tests::a_refusal_is_a_typed_response_not_a_transport_failure'
            File = 'crates/huginn-mind/src/wire.rs'
            Old  = '            Self::Refused(_) | Self::Admit(PipelineAdmissionOutcome::Refused(_)) => "rejected",'
            New  = @'
            Self::Refused(_)
            | Self::Admit(PipelineAdmissionOutcome::Refused(_))
            | Self::Admit(PipelineAdmissionOutcome::AlreadyAdmitted { .. }) => "rejected",
'@
        }
        @{
            Id   = 'D15L'
            Rule = 'A conflict is an answer too: it names the identities that collided, which a rejection would throw away.'
            Test = 'envelope::tests::a_refusal_is_a_typed_response_not_a_transport_failure'
            File = 'crates/huginn-mind/src/wire.rs'
            Old  = '            Self::Refused(_) | Self::Admit(PipelineAdmissionOutcome::Refused(_)) => "rejected",'
            New  = @'
            Self::Refused(_)
            | Self::Admit(PipelineAdmissionOutcome::Refused(_))
            | Self::Admit(PipelineAdmissionOutcome::Conflict { .. }) => "rejected",
'@
        }
        @{
            Id   = 'D16'
            Rule = 'A message of another family is answered, not dropped: CultNet refuses an empty error string, so a silent arm hangs the client.'
            Test = 'serve::tests::a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = @'
        _ => CultNetMessage::Error {
            error: format!(
                "{MIND_SERVICE_ID} answers cultnet.operation_request.v0 and cultnet.schema_catalog_request.v0"
            ),
        },
'@
            New  = @'
        _ => CultNetMessage::Error { error: String::new() },
'@
        }
        @{
            Id   = 'D16L'
            Rule = 'The answer names both messages the service accepts, so a client that guessed wrong learns what to send.'
            Test = 'serve::tests::a_malformed_envelope_is_answered_with_a_failure_and_touches_no_mind'
            File = 'crates/huginn-daemon/src/serve.rs'
            Old  = @'
        _ => CultNetMessage::Error {
            error: format!(
                "{MIND_SERVICE_ID} answers cultnet.operation_request.v0 and cultnet.schema_catalog_request.v0"
            ),
        },
'@
            New  = @'
        _ => CultNetMessage::Error { error: format!("{MIND_SERVICE_ID} answers cultnet.operation_request.v0") },
'@
        }
    )
}
