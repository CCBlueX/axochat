use crate::chat::{
    new_id, now_ms, ChatServer, ClientPacket, Identity, InternalId, RecentReport, ReportView, Scope, SuccessReason,
};
use crate::entity::report;
use crate::error::ClientError;
use crate::store::{LoadReports, Write};
use log::*;

use actix::*;
use uuid::Uuid;

const REPORT_WINDOW: i64 = 60 * 60 * 1000;

/// Reports one user can file per window.
const REPORTS_PER_REPORTER: usize = 10;
const MAX_REASON: usize = 256;
const LISTED_REPORTS: u64 = 50;

impl ChatServer {
    pub(super) fn handle_report(&mut self, user_id: InternalId, query: String, message: Option<u64>, reason: String, ctx: &mut Context<Self>) {
        let Some(reporter) = self.acting_user(user_id) else { return };
        let reason: String = reason.chars().take(MAX_REASON).collect();
        let now = now_ms();
        self.recent_reports.retain(|report| now - report.at < REPORT_WINDOW);
        if self.recent_reports.iter().filter(|report| report.reporter == reporter).count() >= REPORTS_PER_REPORTER {
            self.send_error(user_id, ClientError::RateLimited);
            return;
        }

        self.resolve_user(ctx, query, Scope::Anyone, move |actor, _ctx, resolved| {
            let Some(target) = resolved else {
                actor.send(user_id, ClientPacket::Success { reason: SuccessReason::Report });
                return;
            };
            let target = target.identity.id;
            if target == reporter {
                actor.send_error(user_id, ClientError::NotPermitted);
                return;
            }

            // the reported message must exist, be the target's, and have been readable by the reporter
            let evidence = match message {
                Some(id) => {
                    let recorded = actor.history.iter().find(|recorded| recorded.id == id).filter(|recorded| {
                        recorded.author == target
                            && recorded.audience.as_ref().is_none_or(|audience| audience.contains(&reporter))
                    });
                    match recorded {
                        Some(recorded) => Some((recorded.channel.to_string(), id, recorded.content.clone())),
                        None => {
                            actor.send(user_id, ClientPacket::error_with(ClientError::InvalidId, id.to_string()));
                            return;
                        }
                    }
                }
                None => None,
            };

            // a repeated report is no news for staff
            if actor
                .recent_reports
                .iter()
                .any(|report| report.reporter == reporter && report.target == target && report.message == message)
            {
                actor.send(user_id, ClientPacket::Success { reason: SuccessReason::Report });
                return;
            }

            let model = report::Model {
                id: new_id(&mut actor.rng),
                reporter_id: reporter,
                target_id: target,
                channel: evidence.as_ref().map(|(channel, _, _)| channel.clone()),
                message_id: evidence.as_ref().map(|(_, id, _)| *id as i64),
                content: evidence.map(|(_, _, content)| content),
                reason,
                created_at: now,
                resolved_by: None,
                resolved_at: None,
            };
            info!("`{}` reported `{}`: {}", reporter, target, model.reason);
            actor.persist(vec![Write::Report(model.clone())]);
            actor.send(user_id, ClientPacket::Success { reason: SuccessReason::Report });

            if let Some(report) = actor.report_view(&model) {
                let staff: Vec<InternalId> = actor
                    .connections
                    .iter()
                    .filter(|(_, connection)| connection.user().is_some_and(|user| actor.is_staff(user)))
                    .map(|(id, _)| *id)
                    .collect();
                for id in staff {
                    actor.send_v2(id, ClientPacket::ReportCreated { report: report.clone() });
                }
            }

            actor.recent_reports.push(RecentReport { at: now, target, reporter, message });
        });
    }

    fn report_view(&self, report: &report::Model) -> Option<ReportView> {
        Some(ReportView {
            id: report.id,
            reporter: self.known_ref(report.reporter_id)?,
            target: self.known_ref(report.target_id)?,
            channel: report.channel.clone(),
            message: report.message_id.map(|id| id as u64),
            content: report.content.clone(),
            reason: report.reason.clone(),
            time: report.created_at,
        })
    }

    pub(super) fn handle_request_reports(&mut self, user_id: InternalId, ctx: &mut Context<Self>) {
        if self.staff(user_id).is_none() {
            return;
        }
        let store = self.store.clone();
        ctx.spawn(
            async move { store.send(LoadReports { limit: LISTED_REPORTS }).await }
                .into_actor(self)
                .map(move |result, actor, _ctx| {
                    let Ok(Ok((reports, users))) = result else {
                        error!("Could not load reports");
                        actor.send_error(user_id, ClientError::Internal);
                        return;
                    };
                    for user in &users {
                        actor.directory.entry(user.id).or_insert_with(|| Identity::of(user));
                    }
                    let reports = reports.iter().filter_map(|report| actor.report_view(report)).collect();
                    actor.send_v2(user_id, ClientPacket::Reports { reports });
                }),
        );
    }

    pub(super) fn handle_resolve_report(&mut self, user_id: InternalId, id: Uuid) {
        let Some(staff) = self.staff(user_id) else { return };
        self.persist(vec![Write::ResolveReport { id, by: staff, at: now_ms() }]);
        self.send(user_id, ClientPacket::Success { reason: SuccessReason::Resolve });
    }
}
