//! The real [`ViewSource`]: one desktop API session per page load.

use cancellai_desktop_api::{Client, ClientError, Descriptor, DocumentKind, Query};

use crate::server::ViewSource;
use crate::viewmodel::{DashboardView, build};

/// Fetches the `status` and `plan` documents for a query and builds the view from them.
#[derive(Debug, Clone)]
pub struct ApiViewSource {
    descriptor: Descriptor,
}

impl ApiViewSource {
    pub fn new(descriptor: Descriptor) -> Self {
        Self { descriptor }
    }
}

fn describe(error: ClientError) -> String {
    match error {
        ClientError::Io(error) => format!("desktop API unreachable: {error}"),
        ClientError::Refused { code, message, .. } => {
            format!("desktop API refused the request ({code:?}): {message}")
        }
        ClientError::Protocol(message) => format!("desktop API protocol error: {message}"),
    }
}

impl ViewSource for ApiViewSource {
    fn view(&self, query: Query) -> Result<DashboardView, String> {
        let mut client = Client::connect(&self.descriptor).map_err(describe)?;
        let status = client
            .document(DocumentKind::Status, query)
            .map_err(describe)?;
        let plan = client
            .document(DocumentKind::Plan, query)
            .map_err(describe)?;
        let engine_version = client.engine_version().to_string();
        client.close().map_err(describe)?;
        build(&engine_version, &status, &plan).map_err(|error| error.to_string())
    }
}
