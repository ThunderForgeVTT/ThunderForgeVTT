//! Spec 088 (US2, FR-021, FR-029): the base maps over GraphQL.

use async_graphql::{Context, ID, Object, Result as GraphQLResult, SimpleObject};

use super::{BaseMap, MapCredit, SHARE_ALIKE};
use crate::graphql::{app_state, authenticated_user};

/// Whose a base map is (FR-028, FR-030).
#[derive(SimpleObject, Debug, Clone, PartialEq, Eq)]
#[graphql(name = "MapCredit")]
pub struct GraphQLMapCredit {
    pub author: String,
    pub licence: String,
    pub licence_url: String,
    pub source: String,
    pub catalog: String,
    /// "The copies here are offered under the same licence."
    pub share_alike: String,
}

impl From<&MapCredit> for GraphQLMapCredit {
    fn from(credit: &MapCredit) -> Self {
        GraphQLMapCredit {
            author: credit.author.clone(),
            licence: credit.licence.clone(),
            licence_url: credit.licence_url.clone(),
            source: credit.source.clone(),
            catalog: credit.catalog.clone(),
            share_alike: SHARE_ALIKE.to_string(),
        }
    }
}

/// One map a new world can open on.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "BaseMap")]
pub struct GraphQLBaseMap {
    pub id: ID,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub grid_size: i32,
    pub thumbnail_url: String,
    pub credit: GraphQLMapCredit,
}

impl GraphQLBaseMap {
    fn new(map: &BaseMap, credit: &MapCredit) -> Self {
        GraphQLBaseMap {
            id: ID(map.id.clone()),
            name: map.name.clone(),
            width: i32::try_from(map.width).unwrap_or(i32::MAX),
            height: i32::try_from(map.height).unwrap_or(i32::MAX),
            grid_size: i32::try_from(map.grid_size).unwrap_or(i32::MAX),
            thumbnail_url: map.thumbnail_url(),
            credit: credit.into(),
        }
    }
}

/// The credit a scene's background carries, when it came from a base map.
///
/// Read from the loaded directory, or from the copy compiled into the
/// server when the directory has since gone: a world keeps its copy of the
/// map, so it keeps the credit the licence asks for.
pub fn credit_for(maps: &super::BaseMaps, base_map_id: Option<&str>) -> Option<GraphQLMapCredit> {
    base_map_id?;
    Some(match maps.credit() {
        Some(credit) => credit.into(),
        None => (&MapCredit::bundled()).into(),
    })
}

#[derive(Default)]
pub struct BaseMapQuery;

#[Object]
impl BaseMapQuery {
    /// The maps a new world can open on; empty when the instance has none.
    async fn base_maps(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLBaseMap>> {
        authenticated_user(ctx)?;
        let maps = &app_state(ctx)?.base_maps;
        let Some(credit) = maps.credit() else {
            return Ok(Vec::new());
        };
        Ok(maps
            .list()
            .iter()
            .map(|map| GraphQLBaseMap::new(map, credit))
            .collect())
    }

    /// The map the create-world form starts on, or `null` for **None**.
    async fn default_base_map_id(&self, ctx: &Context<'_>) -> GraphQLResult<Option<ID>> {
        authenticated_user(ctx)?;
        Ok(app_state(ctx)?
            .base_maps
            .default_id()
            .map(|id| ID(id.to_string())))
    }
}
