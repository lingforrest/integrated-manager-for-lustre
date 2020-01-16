// Copyright (c) 2019 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use diesel::prelude::*;
use tokio_diesel::{AsyncRunQueryDsl, OptionalExtension};

/// This async function will retrieve the managed host id for a given fqdn. A future
/// containing the managed host id is returned.
pub async fn get_managed_host_items(
    x: impl ToString,
    pool: &iml_orm::DbPool,
) -> Result<Option<(i32, i32, String)>, tokio_diesel::AsyncError> {
    use iml_orm::schema::chroma_core_managedhost::dsl::*;

    let p = fqdn.eq(x.to_string()).and(not_deleted.eq(true));

    let x = chroma_core_managedhost
        .select((id, content_type_id, state))
        .filter(p)
        .first_async(pool)
        .await
        .optional()?;

    Ok(x.and_then(|(x, y, z): (i32, Option<i32>, String)| Some((x, y?, z))))
}

pub async fn get_active_alert_for_fqdn(
    fqdn: impl ToString,
    record_type: impl ToString,
    pool: &iml_orm::DbPool,
) -> Result<Option<i32>, tokio_diesel::AsyncError> {
    use iml_orm::schema::{chroma_core_alertstate as a, chroma_core_managedhost as mh};

    a::table
        .select(a::dsl::id)
        .inner_join(mh::table.on(a::dsl::alert_item_id.eq(mh::dsl::id.nullable())))
        .filter(
            a::dsl::record_type
                .eq(record_type.to_string())
                .and(a::dsl::active.eq(true)),
        )
        .filter(
            mh::dsl::fqdn
                .eq(fqdn.to_string())
                .and(mh::dsl::not_deleted.eq(true)),
        )
        .first_async(pool)
        .await
        .optional()
}

pub async fn lower_alert(
    alert_id: i32,
    pool: &iml_orm::DbPool,
) -> Result<usize, tokio_diesel::AsyncError> {
    use iml_orm::schema::chroma_core_alertstate::dsl::*;

    let row = chroma_core_alertstate.find(alert_id);

    diesel::update(row)
        .set((active.eq(Option::<bool>::None), end.eq(diesel::dsl::now)))
        .execute_async(pool)
        .await
}

/// This async function will insert a new entry into the chroma_core_alertstate table. This will raise an
/// NtpOutOfSyncAlert alert for a given fqdn.
pub async fn add_alert(
    fqdn: &str,
    item_type_id: i32,
    item_id: i32,
    pool: &iml_orm::DbPool,
) -> Result<usize, tokio_diesel::AsyncError> {
    use iml_orm::schema::chroma_core_alertstate::dsl::*;

    diesel::insert_into(chroma_core_alertstate)
        .values((
            record_type.eq("NtpOutOfSyncAlert"),
            variant.eq("{}"),
            alert_item_id.eq(item_id),
            alert_type.eq("NtpOutOfSyncAlert"),
            begin.eq(diesel::dsl::now),
            message.eq(format!("Ntp is out of sync on server {}", fqdn)),
            active.eq(true),
            dismissed.eq(false),
            severity.eq(40),
            alert_item_type_id.eq(item_type_id),
        ))
        .execute_async(pool)
        .await
}
