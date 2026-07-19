use serde::{Deserialize, Serialize};

pub(crate) const API_NAME: &str = "VTubeStudioPublicAPI";
pub(crate) const API_VERSION: &str = "1.0";
pub(crate) const PLUGIN_NAME: &str = "TTSBard";
pub(crate) const PLUGIN_DEVELOPER: &str = "TTSBard";
pub(crate) const TYPING_PARAMETER_NAME: &str = "TTSBardTyping";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VtsRequest {
    #[serde(rename = "apiName")]
    pub api_name: String,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    #[serde(rename = "requestID")]
    pub request_id: String,
    #[serde(rename = "messageType")]
    pub message_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VtsResponse {
    #[serde(rename = "apiName")]
    pub api_name: String,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    #[serde(rename = "requestID")]
    pub request_id: String,
    #[serde(rename = "messageType")]
    pub message_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VtsErrorData {
    #[serde(rename = "errorID")]
    pub error_id: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuthTokenRequestData {
    #[serde(rename = "pluginName")]
    pub plugin_name: String,
    #[serde(rename = "pluginDeveloper")]
    pub plugin_developer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuthTokenResponseData {
    #[serde(rename = "authenticationToken")]
    pub authentication_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuthenticationRequestData {
    #[serde(rename = "pluginName")]
    pub plugin_name: String,
    #[serde(rename = "pluginDeveloper")]
    pub plugin_developer: String,
    #[serde(rename = "authenticationToken")]
    pub authentication_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuthenticationResponseData {
    pub authenticated: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ParameterCreationRequestData {
    #[serde(rename = "parameterName")]
    pub parameter_name: String,
    pub explanation: String,
    pub min: f64,
    pub max: f64,
    #[serde(rename = "defaultValue")]
    pub default_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ParameterValue {
    pub id: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InjectParameterDataRequestData {
    pub mode: String,
    #[serde(rename = "parameterValues")]
    pub parameter_values: Vec<ParameterValue>,
}

impl VtsRequest {
    pub(crate) fn new(message_type: &str, request_id: &str, data: serde_json::Value) -> Self {
        Self {
            api_name: API_NAME.to_string(),
            api_version: API_VERSION.to_string(),
            request_id: request_id.to_string(),
            message_type: message_type.to_string(),
            data,
        }
    }

    pub(crate) fn auth_token_request(request_id: &str) -> Self {
        let data = serde_json::to_value(AuthTokenRequestData {
            plugin_name: PLUGIN_NAME.to_string(),
            plugin_developer: PLUGIN_DEVELOPER.to_string(),
        })
        .unwrap();
        Self::new("AuthenticationTokenRequest", request_id, data)
    }

    pub(crate) fn authentication_request(request_id: &str, token: &str) -> Self {
        let data = serde_json::to_value(AuthenticationRequestData {
            plugin_name: PLUGIN_NAME.to_string(),
            plugin_developer: PLUGIN_DEVELOPER.to_string(),
            authentication_token: token.to_string(),
        })
        .unwrap();
        Self::new("AuthenticationRequest", request_id, data)
    }

    pub(crate) fn parameter_creation_request(request_id: &str) -> Self {
        let data = serde_json::to_value(ParameterCreationRequestData {
            parameter_name: TYPING_PARAMETER_NAME.to_string(),
            explanation: "TTSBard typing indicator (0=idle, 1=typing)".to_string(),
            min: 0.0,
            max: 1.0,
            default_value: 0.0,
        })
        .unwrap();
        Self::new("ParameterCreationRequest", request_id, data)
    }

    pub(crate) fn inject_parameter_request(request_id: &str, value: f64) -> Self {
        let data = serde_json::to_value(InjectParameterDataRequestData {
            mode: "set".to_string(),
            parameter_values: vec![ParameterValue {
                id: TYPING_PARAMETER_NAME.to_string(),
                value,
            }],
        })
        .unwrap();
        Self::new("InjectParameterDataRequest", request_id, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_token_request_payload() {
        let req = VtsRequest::auth_token_request("req-1");
        let json = serde_json::to_string(&req).unwrap();
        let expected = serde_json::json!({
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "req-1",
            "messageType": "AuthenticationTokenRequest",
            "data": {
                "pluginName": "TTSBard",
                "pluginDeveloper": "TTSBard"
            }
        });
        let actual: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn auth_request_payload() {
        let req = VtsRequest::authentication_request("req-2", "test-token");
        let json = serde_json::to_string(&req).unwrap();
        let expected = serde_json::json!({
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "req-2",
            "messageType": "AuthenticationRequest",
            "data": {
                "pluginName": "TTSBard",
                "pluginDeveloper": "TTSBard",
                "authenticationToken": "test-token"
            }
        });
        let actual: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn parameter_creation_request_payload() {
        let req = VtsRequest::parameter_creation_request("req-3");
        let json = serde_json::to_string(&req).unwrap();
        let expected = serde_json::json!({
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "req-3",
            "messageType": "ParameterCreationRequest",
            "data": {
                "parameterName": "TTSBardTyping",
                "explanation": "TTSBard typing indicator (0=idle, 1=typing)",
                "min": 0.0,
                "max": 1.0,
                "defaultValue": 0.0
            }
        });
        let actual: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn inject_parameter_request_payload() {
        let req = VtsRequest::inject_parameter_request("req-4", 1.0);
        let json = serde_json::to_string(&req).unwrap();
        let expected = serde_json::json!({
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "req-4",
            "messageType": "InjectParameterDataRequest",
            "data": {
                "mode": "set",
                "parameterValues": [
                    { "id": "TTSBardTyping", "value": 1.0 }
                ]
            }
        });
        let actual: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn api_response_deserializes() {
        let json = r#"{
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "req-2",
            "messageType": "APIResponse",
            "data": { "authenticated": true, "reason": "" }
        }"#;
        let resp: VtsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message_type, "APIResponse");
        assert_eq!(resp.request_id, "req-2");
    }

    #[test]
    fn api_error_response_deserializes() {
        let json = r#"{
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "req-2",
            "messageType": "APIError",
            "data": { "errorID": 42, "message": "Token rejected" }
        }"#;
        let resp: VtsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message_type, "APIError");
        let err: VtsErrorData = serde_json::from_value(resp.data).unwrap();
        assert_eq!(err.error_id, 42);
        assert_eq!(err.message, "Token rejected");
    }

    #[test]
    fn auth_token_response_deserializes() {
        let json = r#"{
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "req-1",
            "messageType": "APIResponse",
            "data": { "authenticationToken": "abc123" }
        }"#;
        let resp: VtsResponse = serde_json::from_str(json).unwrap();
        let data: AuthTokenResponseData = serde_json::from_value(resp.data).unwrap();
        assert_eq!(data.authentication_token, "abc123");
    }

    #[test]
    fn rejected_auth_parsed_correctly() {
        let json = r#"{
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "auth-5",
            "messageType": "APIResponse",
            "data": { "authenticated": false, "reason": "User denied request" }
        }"#;
        let resp: VtsResponse = serde_json::from_str(json).unwrap();
        let data: AuthenticationResponseData = serde_json::from_value(resp.data).unwrap();
        assert!(!data.authenticated);
        assert_eq!(data.reason, "User denied request");
    }

    #[test]
    fn inject_param_zero_payload() {
        let req = VtsRequest::inject_parameter_request("inj-0", 0.0);
        let json = serde_json::to_string(&req).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            v["data"]["parameterValues"][0]["value"].as_f64().unwrap(),
            0.0
        );
    }

    #[test]
    fn typed_auth_response_deserializes() {
        let json = r#"{
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "auth-1",
            "messageType": "AuthenticationResponse",
            "data": { "authenticated": true, "reason": "" }
        }"#;
        let resp: VtsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message_type, "AuthenticationResponse");
        assert_eq!(resp.request_id, "auth-1");
        let data: AuthenticationResponseData = serde_json::from_value(resp.data).unwrap();
        assert!(data.authenticated);
    }

    #[test]
    fn typed_parameter_creation_response_deserializes() {
        let json = r#"{
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "param-1",
            "messageType": "ParameterCreationResponse",
            "data": { "parameterName": "TTSBardTyping", "explanation": "Typing indicator" }
        }"#;
        let resp: VtsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message_type, "ParameterCreationResponse");
        assert_eq!(resp.request_id, "param-1");
        assert_eq!(
            resp.data["parameterName"].as_str().unwrap(),
            "TTSBardTyping"
        );
    }

    #[test]
    fn typed_inject_param_response_deserializes() {
        let json = r#"{
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "inj-1",
            "messageType": "InjectParameterDataResponse",
            "data": {}
        }"#;
        let resp: VtsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message_type, "InjectParameterDataResponse");
        assert_eq!(resp.request_id, "inj-1");
    }

    #[test]
    fn mismatched_request_id_still_parses() {
        let json = r#"{
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "unexpected-id",
            "messageType": "AuthenticationResponse",
            "data": { "authenticated": true, "reason": "" }
        }"#;
        let resp: VtsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.request_id, "unexpected-id");
        assert_eq!(resp.message_type, "AuthenticationResponse");
    }

    #[test]
    fn api_error_with_token_message_parses_error_id() {
        let json = r#"{
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": "req-err",
            "messageType": "APIError",
            "data": { "errorID": 42, "message": "auth-token-secret-value" }
        }"#;
        let resp: VtsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message_type, "APIError");
        let err: VtsErrorData = serde_json::from_value(resp.data).unwrap();
        assert_eq!(err.error_id, 42);
    }

    #[test]
    fn test_connection_pulse_sequence() {
        let pulse = VtsRequest::inject_parameter_request("pulse-1", 1.0);
        let pulse_json = serde_json::to_string(&pulse).unwrap();
        let pulse_v: serde_json::Value = serde_json::from_str(&pulse_json).unwrap();
        assert_eq!(
            pulse_v["data"]["parameterValues"][0]["value"]
                .as_f64()
                .unwrap(),
            1.0
        );
        assert_eq!(
            pulse_v["messageType"].as_str().unwrap(),
            "InjectParameterDataRequest"
        );

        let reset = VtsRequest::inject_parameter_request("reset-1", 0.0);
        let reset_json = serde_json::to_string(&reset).unwrap();
        let reset_v: serde_json::Value = serde_json::from_str(&reset_json).unwrap();
        assert_eq!(
            reset_v["data"]["parameterValues"][0]["value"]
                .as_f64()
                .unwrap(),
            0.0
        );
        assert_eq!(
            reset_v["messageType"].as_str().unwrap(),
            "InjectParameterDataRequest"
        );
    }

    #[test]
    fn parameter_creation_id_is_typing_param() {
        let req = VtsRequest::parameter_creation_request("create-1");
        let json = serde_json::to_string(&req).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            v["data"]["parameterName"].as_str().unwrap(),
            "TTSBardTyping"
        );
        assert_eq!(v["data"]["defaultValue"].as_f64().unwrap(), 0.0);
        assert_eq!(v["data"]["min"].as_f64().unwrap(), 0.0);
        assert_eq!(v["data"]["max"].as_f64().unwrap(), 1.0);
    }

    #[test]
    fn inject_param_mode_is_set() {
        let req = VtsRequest::inject_parameter_request("mode-1", 0.5);
        let json = serde_json::to_string(&req).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["data"]["mode"].as_str().unwrap(), "set");
    }
}
