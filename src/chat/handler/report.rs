use super::{ChatServer, ClientPacket};
use crate::chat::history::Audience;
use crate::chat::{
    new_id, now_ms, send_message, Channel, InternalId, RecentReport, ReportView, SuccessReason, UserRef,
};
use crate::entity::report;
use crate::error::ClientError;
use crate::store::{LoadReports, Write};
use log::*;

use actix::*;
use std::collections::HashMap;
use uuid::Uuid;

const REPORT_WINDOW: i64 = 60 * 60 * 1000;
const REPORTS_PER_REPORTER: usize = 10;
const MAX_REASON: usize = 256;
const LISTED_REPORTS: u64 = 50;

const GLOBAL: &str = "global";
const DIRECT: &str = "direct";

impl ChatServer {
    /// A name that matches nobody is answered like a stored report.
    pub(super) fn handle_report(
        &mut self,
        user_id: InternalId,
        query: String,
        message: Option<u64>,
        reason: String,
        ctx: &mut Context<Self>,
    ) {
        let Some(reporter) = self.reporter(user_id) else {
            info!("`{}` is not logged in.", user_id);
            self.send_error(user_id, ClientError::NotLoggedIn);
            return;
        };
        let reason: String = reason.chars().take(MAX_REASON).collect();
        let now = now_ms();
        self.recent_reports.retain(|report| now - report.at < REPORT_WINDOW);
        if self.recent_reports.iter().filter(|report| report.reporter == reporter.id).count() >= REPORTS_PER_REPORTER {
            self.send_error(user_id, ClientError::RateLimited);
            return;
        }

        self.look_up_user(ctx, user_id, query, move |actor, found| {
            let success = ClientPacket::Success { reason: SuccessReason::Report };
            let Some(target) = found.map(|found| found.user) else {
                actor.send(user_id, success);
                return;
            };
            if target.id == reporter.id {
                actor.send_error(user_id, ClientError::NotPermitted);
                return;
            }

            let evidence = match message {
                Some(id) => {
                    let readable = actor
                        .history
                        .get(id)
                        .filter(|recorded| recorded.author == target.id && recorded.audience.reaches(reporter.id));
                    let Some(recorded) = readable else {
                        actor.send_error(user_id, ClientError::InvalidId);
                        return;
                    };
                    let channel = match recorded.audience {
                        Audience::Global => GLOBAL,
                        Audience::Direct { .. } => DIRECT,
                    };
                    Some((channel, id, recorded.original.as_ref().unwrap_or(&recorded.content).clone()))
                }
                None => None,
            };

            let repeated = actor.recent_reports.iter().any(|report| {
                report.reporter == reporter.id && report.target == target.id && report.message == message
            });
            if repeated {
                actor.send(user_id, success);
                return;
            }

            let model = report::Model {
                id: new_id(&mut actor.rng),
                reporter_id: reporter.id,
                target_id: target.id,
                channel: evidence.as_ref().map(|(channel, _, _)| channel.to_string()),
                message_id: message.map(|id| id as i64),
                content: evidence.map(|(_, _, content)| content),
                reason,
                created_at: now,
                resolved_by: None,
                resolved_at: None,
            };
            info!("`{}` reported `{}`: {}", reporter.id, target.id, model.reason);
            actor.persist(vec![Write::Report(model.clone())]);
            actor.recent_reports.push(RecentReport {
                at: now,
                reporter: reporter.id,
                target: target.id,
                message,
            });
            actor.send(user_id, success);

            let report = ClientPacket::ReportCreated {
                report: report_view(&model, reporter, target),
            };
            for (id, session) in &actor.connections {
                if session.hello && actor.is_staff(*id) {
                    send_message(&session.addr, report.clone(), "report");
                }
            }
        });
    }

    fn reporter(&self, user_id: InternalId) -> Option<UserRef> {
        let session = self.connections.get(&user_id)?;
        let id = session.stored_id?;
        Some(UserRef {
            id,
            kind: self.users.get(&id)?.kind,
            name: session.user.as_ref()?.name.clone(),
        })
    }

    pub(super) fn handle_request_reports(&mut self, user_id: InternalId, ctx: &mut Context<Self>) {
        if self.moderator(user_id).is_none() {
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
                    let users: HashMap<Uuid, UserRef> = users.iter().map(|user| (user.id, UserRef::from(user))).collect();
                    let reports = reports
                        .iter()
                        .filter_map(|report| {
                            let reporter = users.get(&report.reporter_id)?.clone();
                            let target = users.get(&report.target_id)?.clone();
                            Some(report_view(report, reporter, target))
                        })
                        .collect();
                    actor.send(user_id, ClientPacket::Reports { reports });
                }),
        );
    }

    pub(super) fn handle_resolve_report(&mut self, user_id: InternalId, id: Uuid) {
        let Some(staff) = self.moderator(user_id) else { return };
        info!("`{}` resolved report `{}`.", staff, id);
        self.persist(vec![Write::ResolveReport { id, by: staff, at: now_ms() }]);
        self.send(user_id, ClientPacket::Success { reason: SuccessReason::Resolve });
    }
}

/// Only the type of the channel is stored: the other side of a reported direct message is the reporter.
fn report_view(report: &report::Model, reporter: UserRef, target: UserRef) -> ReportView {
    let channel = report.channel.as_deref().and_then(|channel| match channel {
        GLOBAL => Some(Channel::Global),
        DIRECT => Some(Channel::Direct { user: reporter.id }),
        _ => None,
    });
    ReportView {
        id: report.id,
        reporter,
        target,
        channel,
        message: report.message_id.map(|id| id as u64),
        content: report.content.clone(),
        reason: report.reason.clone(),
        time: report.created_at,
    }
}
