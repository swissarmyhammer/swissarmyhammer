---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8d80
title: 'RecordingAgent: streaming notifications mis-attributed across prompt calls (off-by-one bucketing)'
---
## Symptom

All 4 existing recordings in `.avp/recordings/` (PostToolUse runs across 2 sessions today + 2 yesterday) show the same off-by-one notification bucketing for the security-rules ruleset:

```
##### no-session-1777392236002791.json (5 calls) #####
  CALL0 initialize
  CALL1 new_session → S9C2SZ
  CALL2 prompt sid=S9C2SZ rule=input-validation     notif=  0  stopReason=end_turn  text_len=0
  CALL3 new_session → FFXX04
  CALL4 prompt sid=FFXX04 rule=no-secrets           notif=198  stopReason=end_turn  text_len=843
         has 2 "status" occurrences in text

##### no-session-1777324249322354.json (5 calls) #####
  CALL2 prompt sid=SM5X88 rule=input-validation     notif=  0  stopReason=end_turn  text_len=0
  CALL4 prompt sid=JPFC6C rule=no-secrets           notif=105  stopReason=end_turn  text_len=436
         has 2 "status" occurrences in text

##### no-session-1777329507538950.json (5 calls) #####
  CALL2 prompt sid=YQPCXY rule=input-validation     notif=  0  stopReason=end_turn  text_len=0
  CALL4 prompt sid=J0JX99 rule=no-secrets           notif=105  stopReason=end_turn  text_len=436
         has 2 "status" occurrences in text
```

Pattern: rule 1 (input-validation) records `stopReason=end_turn` with **0** notifications and **0** chars of agent text. Rule 2 (no-secrets) records 100–200 notifications whose concatenated text contains **both** rules' verdicts:

```
<think></think>
{"status": "passed", "message": "No input validation vulnerabilities found..."}
<think></think>
{"status": "passed", "message": "No hardcoded secrets..."}
```

Confirmed by `validator result` log lines: both rules' verdicts ARE produced and the runner DOES extract them correctly (the log output for input-validation matches the JSON in CALL 4's notifications byte-for-byte). So the model and the runner are fine. The **recording** is wrong.

## Likely root cause

Streaming notifications (token-by-token agent message chunks) arrive on a separate channel from the prompt response. When `prompt(call=2)` returns `end_turn`, its in-flight notifications are still buffered in the recorder's queue. When `prompt(call=3)` (`new_session`) and then `prompt(call=4)` arrive, the recorder hasn't yet drained call 2's notifications, so they land in call 4's bucket instead. By the time call 4's own notifications stream in, they get appended to the same bucket as call 2's.

Mechanism would be in `agent_client_protocol_extras::recording::RecordingAgent` — the call-tracking state probably increments to "current call" before flushing the prior call's notification queue.

## Why this matters

For PostToolUse with 2 rules in 1 ruleset, the two verdicts in the second call's notifications are still readable (just both visible in one place, rule attribution by JSON content). The runner doesn't use recordings, so this didn't cause a regression.

But for **debugging the Stop-hook deadlock** (sibling task `01KQAFFZDX40GSKXQVS0MTNDWV`), the recording is the primary tool. If we can't tell which rule a piece of agent output belongs to, we can't tell *which* rule caused the deadlock or what its last token was. Especially with 11 rules instead of 2, the ambiguity compounds.

## Where to look

- `agent_client_protocol_extras::recording::RecordingAgent` — the notification handler. Specifically: when does `current_call_index` advance? Probably on `new_session` or `prompt` start, before the prior call's notification queue is drained.
- The `Captured ACP notification #N` log lines in `.avp/log` — sequence numbers monotonically increase across all calls in a single session, which means the recorder has one global queue. Bucketing must happen at flush time.

## Suggested fix

When the prompt response returns `end_turn`, hold the prompt call open in the recording until either: (a) some idle timeout passes with no further notifications, or (b) the *next* prompt's first notification arrives. Notifications received in that window belong to the closed call. The async ordering between the response future resolving and the notification stream draining is the bug.

Alternatively: tag each notification with its `sessionId` (already present in notifications, see `n.sessionId`) and route on that, ignoring temporal order.

## Acceptance

- For a 2-rule recording, CALL 2 contains rule-1's notifications (text_len > 0, contains rule-1's JSON verdict only). CALL 4 contains rule-2's notifications only.
- Pre-existing recordings remain readable (don't break the JSON schema).
- Add a unit test in `agent_client_protocol_extras::recording` that simulates two prompt calls with overlapping notification streams (e.g. notifications for call 1 arriving after the call 1 response future resolves) and asserts each call's bucket contains only that call's notifications.

#avp #observability #recording