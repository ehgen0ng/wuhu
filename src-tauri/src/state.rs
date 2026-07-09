use crate::{
    models::{AppState, AppStore},
    steam,
};

pub(crate) fn build_state(store: AppStore) -> Result<AppState, String> {
    Ok(AppState {
        install_status: steam::install_status(&store),
        steam_client: steam::client_status(&store),
        settings: store.settings,
        packages: store.packages,
        tickets: store.tickets,
    })
}
