# On-call handover

The outgoing on-call engineer hands over to the incoming one every Monday at
09:00 UTC, in a short call that follows this checklist. The call should not
last more than 30 minutes.

## Before the call

- Read the alerts of the past week in the alert history, grouped by service.
- List the incidents that are still open, with the owner of each one.
- Check that the incoming engineer can log in to the paging tool and has
  received a test page.
- Write down every change to production that is planned for the coming week.

## During the call

1. Walk through each open incident and agree on its next step.
2. Name the alerts that fired more than three times, and say whether they
   need tuning.
3. Hand over the maintenance planned for the coming week, with the change
   ticket of each operation.
4. Confirm the escalation contact for the week: the team lead on duty.

## After the call

- The incoming engineer acknowledges the handover in the `#ops-handover`
  channel, with a link to the notes of the call.
- Update the on-call calendar if a swap was agreed during the call.
- Every alert that needs tuning gets a ticket before the end of the day.

## If the handover cannot happen

If the incoming engineer is unavailable on Monday, the outgoing one stays on
call until a replacement is named, and the team lead records the swap in the
on-call calendar. A handover is never skipped: when the call cannot take
place, the checklist is written down and sent to the incoming engineer
instead.
