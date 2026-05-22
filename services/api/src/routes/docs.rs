use axum::{
    extract::State,
    response::{Html, IntoResponse},
    Json,
};
use serde_json::{json, Value};

use crate::state::AppState;

pub async fn api_docs() -> impl IntoResponse {
    Html(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>IssueHub API Docs</title>
    <link rel="stylesheet" href="https://unpkg.com/@stoplight/elements/styles.min.css">
    <style>
      body { margin: 0; background: #ffffff; }
      .sl-elements-api { height: 100vh; }
    </style>
  </head>
  <body>
    <elements-api
      apiDescriptionUrl="/api-docs/openapi.json"
      router="hash"
      layout="sidebar"
      tryItCredentialsPolicy="same-origin"
    />
    <script src="https://unpkg.com/@stoplight/elements/web-components.min.js"></script>
  </body>
</html>"#,
    )
}

pub async fn openapi_spec(State(state): State<AppState>) -> Json<Value> {
    let _ = state.config;

    Json(json!({
      "openapi": "3.1.0",
      "info": {
        "title": "IssueHub API",
        "version": "0.1.0",
        "description": "IssueHub API for local issue management with optional GitLab integration, local ACL, attachment proxy/storage and PostgreSQL-backed background processing."
      },
      "servers": [{ "url": "/" }],
      "tags": [
        { "name": "Health" },
        { "name": "Auth" },
        { "name": "Projects" },
        { "name": "Users" },
        { "name": "Issues" },
        { "name": "Attachments" },
        { "name": "Jobs" }
      ],
      "paths": {
        "/health": {
          "get": {
            "tags": ["Health"],
            "summary": "Service health",
            "responses": {
              "200": {
                "description": "Service health",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                  }
                }
              }
            }
          }
        },
        "/health/live": {
          "get": {
            "tags": ["Health"],
            "summary": "API liveness",
            "responses": {
              "200": {
                "description": "API process is alive",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                  }
                }
              }
            }
          }
        },
        "/health/ready": {
          "get": {
            "tags": ["Health"],
            "summary": "Readiness over DB, schema, queue and worker heartbeat",
            "responses": {
              "200": {
                "description": "Service is ready",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ReadinessResponse" }
                  }
                }
              },
              "503": {
                "description": "Service is not ready",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ReadinessResponse" }
                  }
                }
              }
            }
          }
        },
        "/metrics": {
          "get": {
            "tags": ["Health"],
            "summary": "Prometheus metrics",
            "responses": {
              "200": {
                "description": "Prometheus text metrics",
                "content": {
                  "text/plain": {
                    "schema": { "type": "string" }
                  }
                }
              },
              "503": {
                "description": "Metrics rendered while DB is unavailable",
                "content": {
                  "text/plain": {
                    "schema": { "type": "string" }
                  }
                }
              }
            }
          }
        },
        "/api/v1/auth/login": {
          "post": {
            "tags": ["Auth"],
            "summary": "Create authenticated session",
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/LoginRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Authenticated user and session cookie",
                "headers": {
                  "Set-Cookie": {
                    "schema": { "type": "string" },
                    "description": "HttpOnly session cookie"
                  }
                },
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                  }
                }
              },
              "401": {
                "description": "Invalid credentials",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/v1/auth/me": {
          "get": {
            "tags": ["Auth"],
            "summary": "Get current authenticated user",
            "security": [{ "sessionCookie": [] }],
            "responses": {
              "200": {
                "description": "Authenticated user",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                  }
                }
              },
              "401": {
                "description": "Missing or invalid session",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              }
            }
          },
          "patch": {
            "tags": ["Auth"],
            "summary": "Update current authenticated user profile",
            "security": [{ "sessionCookie": [] }],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UpdateMeRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Updated authenticated user",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" }
            }
          }
        },
        "/api/v1/auth/change-password": {
          "post": {
            "tags": ["Auth"],
            "summary": "Change password for current authenticated user",
            "security": [{ "sessionCookie": [] }],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/ChangePasswordRequest" }
                }
              }
            },
            "responses": {
              "204": { "description": "Password changed" },
              "400": { "$ref": "#/components/responses/BadRequestError" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" }
            }
          }
        },
        "/api/v1/auth/logout": {
          "post": {
            "tags": ["Auth"],
            "summary": "Clear authenticated session",
            "security": [{ "sessionCookie": [] }],
            "responses": {
              "204": {
                "description": "Session cleared"
              }
            }
          }
        },
        "/api/v1/auth/password-recovery": {
          "post": {
            "tags": ["Auth"],
            "summary": "Request password recovery email",
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/PasswordRecoveryRequest" }
                }
              }
            },
            "responses": {
              "204": { "description": "Recovery email queued if account exists" }
            }
          }
        },
        "/api/v1/auth/password-recovery/{token}": {
          "get": {
            "tags": ["Auth"],
            "summary": "Preview password recovery token",
            "parameters": [token_parameter("token")],
            "responses": {
              "200": {
                "description": "Valid recovery token preview",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/PasswordRecoveryPreview" }
                  }
                }
              },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/auth/password-recovery/{token}/reset": {
          "post": {
            "tags": ["Auth"],
            "summary": "Reset password using recovery token",
            "parameters": [token_parameter("token")],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/PasswordResetRequest" }
                }
              }
            },
            "responses": {
              "204": { "description": "Password reset completed" },
              "400": { "$ref": "#/components/responses/BadRequestError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/invitations/{invite_token}": {
          "get": {
            "tags": ["Auth"],
            "summary": "Get public invitation preview",
            "parameters": [
              {
                "name": "invite_token",
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
              }
            ],
            "responses": {
              "200": {
                "description": "Invitation preview",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/InvitationPreview" }
                  }
                }
              },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/invitations/{invite_token}/accept": {
          "post": {
            "tags": ["Auth"],
            "summary": "Accept invitation and create account",
            "parameters": [
              {
                "name": "invite_token",
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
              }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/AcceptInvitationRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Account created and authenticated session started",
                "headers": {
                  "Set-Cookie": {
                    "schema": { "type": "string" },
                    "description": "HttpOnly session cookie"
                  }
                },
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                  }
                }
              },
              "400": {
                "description": "Invitation invalid or expired",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/overview": {
          "get": {
            "tags": ["Health"],
            "summary": "Overview of projects, issues and queue state",
            "security": [{ "sessionCookie": [] }],
            "responses": {
              "200": {
                "description": "Current service overview",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/OverviewResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" }
            }
          }
        },
        "/api/v1/projects": {
          "get": {
            "tags": ["Projects"],
            "summary": "List internal projects and GitLab integration state",
            "security": [{ "sessionCookie": [] }],
            "responses": {
              "200": {
                "description": "Project list",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/Project" }
                    }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" }
            }
          },
          "post": {
            "tags": ["Projects"],
            "summary": "Create internal project",
            "security": [{ "sessionCookie": [] }],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateProjectRequest" }
                }
              }
            },
            "responses": {
              "201": {
                "description": "Project created",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/Project" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" }
            }
          }
        },
        "/api/v1/projects/{project_id}": {
          "patch": {
            "tags": ["Projects"],
            "summary": "Update internal project",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UpdateProjectRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Updated project",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/Project" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          },
          "delete": {
            "tags": ["Projects"],
            "summary": "Delete internal project",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "responses": {
              "204": { "description": "Project deleted" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/projects/{project_id}/gitlab-integration": {
          "post": {
            "tags": ["Projects"],
            "summary": "Create or update GitLab integration for a project",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UpsertProjectGitLabIntegrationRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Updated project with integration metadata",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/Project" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          },
          "delete": {
            "tags": ["Projects"],
            "summary": "Delete GitLab integration for a project",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "responses": {
              "204": { "description": "Integration deleted" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/projects/{project_id}/gitlab-integration/validate": {
          "post": {
            "tags": ["Projects"],
            "summary": "Validate GitLab integration settings against GitLab API",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/ValidateProjectGitLabIntegrationRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Integration settings validated successfully",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/GitLabIntegrationValidationResponse" }
                  }
                }
              },
              "400": {
                "description": "Validation input is incomplete",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "502": {
                "description": "GitLab API validation failed",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/v1/projects/{project_id}/gitlab-integration/import": {
          "post": {
            "tags": ["Projects"],
            "summary": "Import issues from the configured GitLab project into the local registry",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "responses": {
              "200": {
                "description": "Import completed",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/GitLabIssueImportResponse" }
                  }
                }
              },
              "400": {
                "description": "Integration is missing token or sync is disabled",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" },
              "502": {
                "description": "GitLab API import failed",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/v1/gitlab/webhooks/{project_id}": {
          "post": {
            "tags": ["GitLab"],
            "summary": "Receive a GitLab webhook for a configured project integration",
            "parameters": [
              project_id_parameter(),
              {
                "name": "X-Gitlab-Token",
                "in": "header",
                "required": true,
                "schema": { "type": "string" }
              },
              {
                "name": "X-Gitlab-Event",
                "in": "header",
                "required": false,
                "schema": { "type": "string", "example": "Issue Hook" }
              }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": {
                    "type": "object",
                    "description": "GitLab webhook payload"
                  }
                }
              }
            },
            "responses": {
              "202": {
                "description": "Webhook accepted",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/GitLabWebhookResponse" }
                  }
                }
              },
              "400": {
                "description": "Webhook payload does not match the configured integration",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "401": {
                "description": "Invalid or missing webhook token",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/projects/{project_id}/access": {
          "get": {
            "tags": ["Projects"],
            "summary": "Admin view of project-level access assignments",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "responses": {
              "200": {
                "description": "Project access overview",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ProjectAccessOverview" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          },
          "put": {
            "tags": ["Projects"],
            "summary": "Admin replace of project-level access assignments",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UpdateProjectAccessRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Updated project access overview",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ProjectAccessOverview" }
                  }
                }
              },
              "400": { "$ref": "#/components/responses/BadRequestError" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/users": {
          "get": {
            "tags": ["Users"],
            "summary": "Admin overview of users and invitations",
            "security": [{ "sessionCookie": [] }],
            "responses": {
              "200": {
                "description": "Users and invitations",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UserManagementOverview" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" }
            }
          }
        },
        "/api/v1/users/{user_id}": {
          "patch": {
            "tags": ["Users"],
            "summary": "Admin update of user role and activation",
            "security": [{ "sessionCookie": [] }],
            "parameters": [
              {
                "name": "user_id",
                "in": "path",
                "required": true,
                "schema": { "type": "string", "format": "uuid" }
              }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UpdateUserRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Updated user",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ManagedUser" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/users/{user_id}/access": {
          "get": {
            "tags": ["Users"],
            "summary": "Admin view of project and direct issue access for one user",
            "security": [{ "sessionCookie": [] }],
            "parameters": [
              {
                "name": "user_id",
                "in": "path",
                "required": true,
                "schema": { "type": "string", "format": "uuid" }
              }
            ],
            "responses": {
              "200": {
                "description": "User access overview",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UserAccessOverview" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          },
          "put": {
            "tags": ["Users"],
            "summary": "Admin replace of project and direct issue access for one user",
            "security": [{ "sessionCookie": [] }],
            "parameters": [
              {
                "name": "user_id",
                "in": "path",
                "required": true,
                "schema": { "type": "string", "format": "uuid" }
              }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UpdateUserAccessRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Updated user access overview",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UserAccessOverview" }
                  }
                }
              },
              "400": {
                "description": "Invalid access assignment payload",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/users/invitations": {
          "post": {
            "tags": ["Users"],
            "summary": "Create or resend invitation email",
            "security": [{ "sessionCookie": [] }],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateInvitationRequest" }
                }
              }
            },
            "responses": {
              "201": {
                "description": "Invitation created and queued for email sending",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UserInvitation" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" }
            }
          }
        },
        "/api/v1/users/invitations/{invitation_id}": {
          "delete": {
            "tags": ["Users"],
            "summary": "Delete pending invitation",
            "security": [{ "sessionCookie": [] }],
            "parameters": [uuid_parameter("invitation_id")],
            "responses": {
              "204": { "description": "Invitation deleted" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/users/invitations/{invitation_id}/resend": {
          "post": {
            "tags": ["Users"],
            "summary": "Resend pending invitation email",
            "security": [{ "sessionCookie": [] }],
            "parameters": [uuid_parameter("invitation_id")],
            "responses": {
              "200": {
                "description": "Invitation resent",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/UserInvitation" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/issues": {
          "get": {
            "tags": ["Issues"],
            "summary": "List proxied issues across internal projects",
            "security": [{ "sessionCookie": [] }],
            "responses": {
              "200": {
                "description": "Issue list",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/Issue" }
                    }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" }
            }
          }
        },
        "/api/v1/projects/{project_id}/issues": {
          "post": {
            "tags": ["Issues"],
            "summary": "Create a new issue in IssueHub, optionally synchronized to the project's GitLab integration",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateProjectIssueRequest" }
                }
              }
            },
            "responses": {
              "201": {
                "description": "Issue created and added to local registry",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/Issue" }
                  }
                }
              },
              "400": {
                "description": "Issue input invalid or integration disabled",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" },
              "502": {
                "description": "GitLab issue creation failed",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/v1/projects/{project_id}/uploads": {
          "post": {
            "tags": ["Attachments"],
            "summary": "Upload a temporary attachment for issue description draft",
            "security": [{ "sessionCookie": [] }],
            "parameters": [project_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "multipart/form-data": {
                  "schema": {
                    "type": "object",
                    "required": ["file"],
                    "properties": {
                      "file": { "type": "string", "format": "binary" }
                    }
                  }
                }
              }
            },
            "responses": {
              "201": {
                "description": "Temporary upload created",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/IssueUpload" }
                  }
                }
              },
              "400": { "$ref": "#/components/responses/BadRequestError" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/issues/{issue_id}": {
          "get": {
            "tags": ["Issues"],
            "summary": "Get issue detail, comments and attachment references",
            "security": [{ "sessionCookie": [] }],
            "parameters": [issue_id_parameter()],
            "responses": {
              "200": {
                "description": "Issue detail",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/IssueDetail" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          },
          "patch": {
            "tags": ["Issues"],
            "summary": "Update issue title, description or state",
            "security": [{ "sessionCookie": [] }],
            "parameters": [issue_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UpdateIssueRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Issue updated",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/Issue" }
                  }
                }
              },
              "400": { "$ref": "#/components/responses/BadRequestError" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" },
              "502": {
                "description": "GitLab issue update failed",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/v1/issues/{issue_id}/comments": {
          "post": {
            "tags": ["Issues"],
            "summary": "Create a new comment in GitLab and persist it locally",
            "security": [{ "sessionCookie": [] }],
            "parameters": [issue_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateIssueCommentRequest" }
                }
              }
            },
            "responses": {
              "201": {
                "description": "Comment created",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/Comment" }
                  }
                }
              },
              "400": {
                "description": "Comment body invalid or integration disabled",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" },
              "502": {
                "description": "GitLab comment creation failed",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/v1/issues/{issue_id}/comments/sync": {
          "post": {
            "tags": ["Issues"],
            "summary": "Admin-triggered sync of GitLab comments for a single issue",
            "security": [{ "sessionCookie": [] }],
            "parameters": [issue_id_parameter()],
            "responses": {
              "200": {
                "description": "Comment sync result",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/GitLabCommentImportResponse" }
                  }
                }
              },
              "400": {
                "description": "Integration disabled or token missing",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" },
              "502": {
                "description": "GitLab comment sync failed",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/v1/issues/{issue_id}/uploads": {
          "post": {
            "tags": ["Attachments"],
            "summary": "Upload a temporary attachment for issue comments",
            "security": [{ "sessionCookie": [] }],
            "parameters": [issue_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "multipart/form-data": {
                  "schema": {
                    "type": "object",
                    "required": ["file"],
                    "properties": {
                      "file": { "type": "string", "format": "binary" }
                    }
                  }
                }
              }
            },
            "responses": {
              "201": {
                "description": "Temporary upload created",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/IssueUpload" }
                  }
                }
              },
              "400": { "$ref": "#/components/responses/BadRequestError" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/issues/{issue_id}/access": {
          "get": {
            "tags": ["Issues"],
            "summary": "Admin view of per-issue user access assignments",
            "security": [{ "sessionCookie": [] }],
            "parameters": [issue_id_parameter()],
            "responses": {
              "200": {
                "description": "Issue access overview",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/IssueAccessOverview" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          },
          "put": {
            "tags": ["Issues"],
            "summary": "Admin replace of per-issue user access assignments",
            "security": [{ "sessionCookie": [] }],
            "parameters": [issue_id_parameter()],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/UpdateIssueAccessRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Updated issue access overview",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/IssueAccessOverview" }
                  }
                }
              },
              "400": {
                "description": "Invalid access assignment payload",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/attachments/{attachment_id}/download": {
          "get": {
            "tags": ["Attachments"],
            "summary": "Proxy attachment download through IssueHub",
            "security": [{ "sessionCookie": [] }],
            "parameters": [
              {
                "name": "attachment_id",
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
              }
            ],
            "responses": {
              "200": {
                "description": "Attachment content streamed through IssueHub"
              },
              "400": { "$ref": "#/components/responses/BadRequestError" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "404": { "$ref": "#/components/responses/NotFoundError" },
              "502": {
                "description": "GitLab attachment fetch failed",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/api/v1/uploads/{upload_id}": {
          "delete": {
            "tags": ["Attachments"],
            "summary": "Delete an unconsumed temporary upload",
            "security": [{ "sessionCookie": [] }],
            "parameters": [uuid_parameter("upload_id")],
            "responses": {
              "204": { "description": "Upload deleted" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/uploads/{upload_id}/download": {
          "get": {
            "tags": ["Attachments"],
            "summary": "Download temporary upload content through IssueHub",
            "security": [{ "sessionCookie": [] }],
            "parameters": [uuid_parameter("upload_id")],
            "responses": {
              "200": { "description": "Upload content streamed through IssueHub" },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" },
              "404": { "$ref": "#/components/responses/NotFoundError" }
            }
          }
        },
        "/api/v1/admin/health": {
          "get": {
            "tags": ["Health"],
            "summary": "Admin operational health overview",
            "security": [{ "sessionCookie": [] }],
            "responses": {
              "200": {
                "description": "Operational health details",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/AdminHealthResponse" }
                  }
                }
              },
              "503": {
                "description": "Operational health is degraded",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/AdminHealthResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" }
            }
          }
        },
        "/api/v1/jobs": {
          "get": {
            "tags": ["Jobs"],
            "summary": "List recent background jobs",
            "security": [{ "sessionCookie": [] }],
            "responses": {
              "200": {
                "description": "Recent jobs",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/Job" }
                    }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" },
              "403": { "$ref": "#/components/responses/ForbiddenError" }
            }
          },
          "post": {
            "tags": ["Jobs"],
            "summary": "Enqueue background job in PostgreSQL queue",
            "security": [{ "sessionCookie": [] }],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/EnqueueJobRequest" }
                }
              }
            },
            "responses": {
              "202": {
                "description": "Job accepted",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/JobResponse" }
                  }
                }
              },
              "401": { "$ref": "#/components/responses/UnauthorizedError" }
            }
          }
        }
      },
      "components": {
        "securitySchemes": {
          "sessionCookie": {
            "type": "apiKey",
            "in": "cookie",
            "name": "issuehub_session"
          }
        },
        "responses": {
          "BadRequestError": {
            "description": "Request payload is invalid",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/ErrorResponse" }
              }
            }
          },
          "UnauthorizedError": {
            "description": "Missing or invalid session",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/ErrorResponse" }
              }
            }
          },
          "NotFoundError": {
            "description": "Requested entity was not found",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/ErrorResponse" }
              }
            }
          },
          "ForbiddenError": {
            "description": "User does not have enough privileges",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/ErrorResponse" }
              }
            }
          }
        },
        "schemas": {
          "HealthResponse": {
            "type": "object",
            "required": ["status", "service"],
            "properties": {
              "status": { "type": "string", "example": "ok" },
              "service": { "type": "string", "example": "api" }
            }
          },
          "HealthCheck": {
            "type": "object",
            "required": ["name", "status"],
            "properties": {
              "name": { "type": "string", "example": "worker" },
              "status": { "type": "string", "enum": ["ok", "down"] },
              "message": { "type": ["string", "null"], "example": "1/1 workers healthy" }
            }
          },
          "ReadinessResponse": {
            "type": "object",
            "required": ["status", "service", "checks"],
            "properties": {
              "status": { "type": "string", "enum": ["ok", "degraded"] },
              "service": { "type": "string", "example": "api" },
              "checks": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/HealthCheck" }
              }
            }
          },
          "QueueHealth": {
            "type": "object",
            "required": [
              "pending_jobs",
              "processing_jobs",
              "done_jobs",
              "dead_jobs",
              "stale_processing_jobs",
              "oldest_pending_seconds",
              "smtp_failed_jobs",
              "webhook_failed_jobs"
            ],
            "properties": {
              "pending_jobs": { "type": "integer", "example": 0 },
              "processing_jobs": { "type": "integer", "example": 0 },
              "done_jobs": { "type": "integer", "example": 12 },
              "dead_jobs": { "type": "integer", "example": 0 },
              "stale_processing_jobs": { "type": "integer", "example": 0 },
              "oldest_pending_seconds": { "type": "integer", "example": 0 },
              "smtp_failed_jobs": { "type": "integer", "example": 0 },
              "webhook_failed_jobs": { "type": "integer", "example": 0 }
            }
          },
          "WorkerHeartbeat": {
            "type": "object",
            "required": [
              "worker_id",
              "status",
              "healthy",
              "heartbeat_age_seconds",
              "heartbeat_at",
              "processed_jobs",
              "failed_jobs"
            ],
            "properties": {
              "worker_id": { "type": "string", "example": "worker-9d1f2c6c-6d1d-4d38-8d56-2d76e7bbd409" },
              "status": { "type": "string", "example": "idle" },
              "healthy": { "type": "boolean" },
              "heartbeat_age_seconds": { "type": "integer", "example": 3 },
              "heartbeat_at": { "type": "string", "format": "date-time" },
              "last_job_id": { "type": ["string", "null"], "format": "uuid" },
              "last_job_topic": { "type": ["string", "null"], "example": "gitlab.webhook.received" },
              "last_error": { "type": ["string", "null"] },
              "processed_jobs": { "type": "integer", "example": 42 },
              "failed_jobs": { "type": "integer", "example": 1 }
            }
          },
          "RecentJobFailure": {
            "type": "object",
            "required": ["id", "topic", "status", "attempt_count", "updated_at"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "topic": { "type": "string", "example": "user.invitation.send_email" },
              "status": { "type": "string", "example": "dead" },
              "attempt_count": { "type": "integer", "example": 5 },
              "last_error": { "type": ["string", "null"] },
              "updated_at": { "type": "string", "format": "date-time" }
            }
          },
          "AdminHealthResponse": {
            "type": "object",
            "required": ["status", "service", "generated_at", "checks", "queue", "workers", "recent_failed_jobs"],
            "properties": {
              "status": { "type": "string", "enum": ["ok", "degraded"] },
              "service": { "type": "string", "example": "api" },
              "generated_at": { "type": "string", "format": "date-time" },
              "checks": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/HealthCheck" }
              },
              "queue": { "$ref": "#/components/schemas/QueueHealth" },
              "workers": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/WorkerHeartbeat" }
              },
              "recent_failed_jobs": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/RecentJobFailure" }
              }
            }
          },
          "ErrorResponse": {
            "type": "object",
            "required": ["message"],
            "properties": {
              "message": { "type": "string", "example": "Invalid credentials" }
            }
          },
          "OverviewResponse": {
            "type": "object",
            "required": ["project_count", "integrated_project_count", "issue_count", "pending_jobs", "processing_jobs"],
            "properties": {
              "project_count": { "type": "integer", "example": 1 },
              "integrated_project_count": { "type": "integer", "example": 1 },
              "issue_count": { "type": "integer", "example": 2 },
              "pending_jobs": { "type": "integer", "example": 0 },
              "processing_jobs": { "type": "integer", "example": 0 }
            }
          },
          "User": {
            "type": "object",
            "required": ["id", "email", "full_name", "is_admin"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "email": { "type": "string", "format": "email" },
              "full_name": { "type": "string" },
              "is_admin": { "type": "boolean" }
            }
          },
          "AuthResponse": {
            "type": "object",
            "required": ["user"],
            "properties": {
              "user": { "$ref": "#/components/schemas/User" }
            }
          },
          "UpdateMeRequest": {
            "type": "object",
            "required": ["full_name"],
            "properties": {
              "full_name": { "type": "string" },
              "preferred_language": { "type": ["string", "null"], "enum": ["cs", "en", null] }
            }
          },
          "LoginRequest": {
            "type": "object",
            "required": ["email", "password"],
            "properties": {
              "email": { "type": "string", "format": "email", "example": "admin@example.com" },
              "password": { "type": "string", "example": "admin1234" }
            }
          },
          "ChangePasswordRequest": {
            "type": "object",
            "required": ["current_password", "new_password"],
            "properties": {
              "current_password": { "type": "string" },
              "new_password": { "type": "string" }
            }
          },
          "PasswordRecoveryRequest": {
            "type": "object",
            "required": ["email"],
            "properties": {
              "email": { "type": "string", "format": "email" }
            }
          },
          "PasswordRecoveryPreview": {
            "type": "object",
            "required": ["email", "expires_at"],
            "properties": {
              "email": { "type": "string", "format": "email" },
              "expires_at": { "type": "string", "format": "date-time" }
            }
          },
          "PasswordResetRequest": {
            "type": "object",
            "required": ["password"],
            "properties": {
              "password": { "type": "string" }
            }
          },
          "InvitationPreview": {
            "type": "object",
            "required": ["email", "is_admin", "status", "expires_at"],
            "properties": {
              "email": { "type": "string", "format": "email" },
              "is_admin": { "type": "boolean" },
              "status": { "type": "string", "example": "pending" },
              "expires_at": { "type": "string", "format": "date-time" }
            }
          },
          "AcceptInvitationRequest": {
            "type": "object",
            "required": ["full_name", "password"],
            "properties": {
              "full_name": { "type": "string", "example": "John Doe" },
              "password": { "type": "string", "example": "correct horse battery staple" }
            }
          },
          "ManagedUser": {
            "type": "object",
            "required": ["id", "email", "full_name", "is_admin", "active", "created_at"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "email": { "type": "string", "format": "email" },
              "full_name": { "type": "string" },
              "is_admin": { "type": "boolean" },
              "active": { "type": "boolean" },
              "created_at": { "type": "string", "format": "date-time" }
            }
          },
          "UserInvitation": {
            "type": "object",
            "required": ["id", "email", "is_admin", "status", "expires_at", "last_sent_at", "created_at"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "email": { "type": "string", "format": "email" },
              "is_admin": { "type": "boolean" },
              "status": { "type": "string", "example": "pending" },
              "expires_at": { "type": "string", "format": "date-time" },
              "last_sent_at": { "type": "string", "format": "date-time" },
              "accepted_at": { "type": ["string", "null"], "format": "date-time" },
              "created_at": { "type": "string", "format": "date-time" }
            }
          },
          "UserManagementOverview": {
            "type": "object",
            "required": ["users", "invitations"],
            "properties": {
              "users": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/ManagedUser" }
              },
              "invitations": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/UserInvitation" }
              }
            }
          },
          "UpdateUserRequest": {
            "type": "object",
            "required": ["full_name", "is_admin", "active"],
            "properties": {
              "full_name": { "type": "string" },
              "is_admin": { "type": "boolean" },
              "active": { "type": "boolean" }
            }
          },
          "UserAccessProjectPermission": {
            "type": "object",
            "required": ["project_id", "project_name", "permission"],
            "properties": {
              "project_id": { "type": "string", "format": "uuid" },
              "project_name": { "type": "string" },
              "permission": { "type": "string", "example": "view" }
            }
          },
          "UserAccessIssuePermission": {
            "type": "object",
            "required": ["issue_id", "issue_title", "gitlab_issue_iid", "project_id", "project_name", "permission"],
            "properties": {
              "issue_id": { "type": "string", "format": "uuid" },
              "issue_title": { "type": "string" },
              "gitlab_issue_iid": { "type": "integer", "example": 42 },
              "project_id": { "type": "string", "format": "uuid" },
              "project_name": { "type": "string" },
              "permission": { "type": "string", "example": "comment" }
            }
          },
          "UserAccessProjectOption": {
            "type": "object",
            "required": ["project_id", "project_name"],
            "properties": {
              "project_id": { "type": "string", "format": "uuid" },
              "project_name": { "type": "string" }
            }
          },
          "UserAccessIssueOption": {
            "type": "object",
            "required": ["issue_id", "issue_title", "gitlab_issue_iid", "project_id", "project_name"],
            "properties": {
              "issue_id": { "type": "string", "format": "uuid" },
              "issue_title": { "type": "string" },
              "gitlab_issue_iid": { "type": "integer", "example": 42 },
              "project_id": { "type": "string", "format": "uuid" },
              "project_name": { "type": "string" }
            }
          },
          "UserAccessOverview": {
            "type": "object",
            "required": ["project_permissions", "issue_permissions", "available_projects", "available_issues"],
            "properties": {
              "project_permissions": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/UserAccessProjectPermission" }
              },
              "issue_permissions": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/UserAccessIssuePermission" }
              },
              "available_projects": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/UserAccessProjectOption" }
              },
              "available_issues": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/UserAccessIssueOption" }
              }
            }
          },
          "UpdateUserAccessRequest": {
            "type": "object",
            "required": ["project_permissions"],
            "properties": {
              "project_permissions": {
                "type": "array",
                "items": {
                  "type": "object",
                  "required": ["project_id", "permission"],
                  "properties": {
                    "project_id": { "type": "string", "format": "uuid" },
                    "permission": { "type": "string", "example": "create_issue" }
                  }
                }
              },
              "issue_permissions": {
                "type": "array",
                "items": {
                  "type": "object",
                  "required": ["issue_id", "permission"],
                  "properties": {
                    "issue_id": { "type": "string", "format": "uuid" },
                    "permission": { "type": "string", "example": "comment" }
                  }
                }
              }
            }
          },
          "CreateInvitationRequest": {
            "type": "object",
            "required": ["email", "is_admin"],
            "properties": {
              "email": { "type": "string", "format": "email" },
              "is_admin": { "type": "boolean" }
            }
          },
          "ProjectIntegration": {
            "type": "object",
            "required": ["id", "gitlab_base_url", "gitlab_api_base_url", "gitlab_project_id", "verify_tls", "sync_enabled"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "gitlab_base_url": { "type": "string" },
              "gitlab_api_base_url": { "type": "string" },
              "gitlab_project_id": { "type": "integer" },
              "verify_tls": { "type": "boolean" },
              "sync_enabled": { "type": "boolean" }
            }
          },
          "Project": {
            "type": "object",
            "required": ["id", "slug", "name", "description", "active", "capabilities"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "slug": { "type": "string" },
              "name": { "type": "string" },
              "description": { "type": "string" },
              "active": { "type": "boolean" },
              "capabilities": { "$ref": "#/components/schemas/ProjectCapabilities" },
              "gitlab_integration": {
                "oneOf": [
                  { "$ref": "#/components/schemas/ProjectIntegration" },
                  { "type": "null" }
                ]
              }
            }
          },
          "ProjectCapabilities": {
            "type": "object",
            "required": ["can_view", "can_create_issue", "can_manage"],
            "properties": {
              "can_view": { "type": "boolean" },
              "can_create_issue": { "type": "boolean" },
              "can_manage": { "type": "boolean" }
            }
          },
          "ProjectAccessAssignment": {
            "type": "object",
            "required": ["subject_type", "subject_id", "display_name", "email", "permission"],
            "properties": {
              "subject_type": { "type": "string", "enum": ["user", "email"] },
              "subject_id": { "type": "string" },
              "display_name": { "type": "string" },
              "email": { "type": "string", "format": "email" },
              "permission": { "type": "string", "example": "create_issue" }
            }
          },
          "ProjectAccessSubjectOption": {
            "type": "object",
            "required": ["subject_type", "subject_id", "display_name", "email"],
            "properties": {
              "subject_type": { "type": "string", "enum": ["user", "email"] },
              "subject_id": { "type": "string" },
              "display_name": { "type": "string" },
              "email": { "type": "string", "format": "email" }
            }
          },
          "ProjectAccessOverview": {
            "type": "object",
            "required": ["assignments", "available_subjects"],
            "properties": {
              "assignments": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/ProjectAccessAssignment" }
              },
              "available_subjects": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/ProjectAccessSubjectOption" }
              }
            }
          },
          "UpdateProjectAccessRequest": {
            "type": "object",
            "required": ["assignments"],
            "properties": {
              "assignments": {
                "type": "array",
                "items": {
                  "type": "object",
                  "required": ["subject_type", "subject_id", "permission"],
                  "properties": {
                    "subject_type": { "type": "string", "enum": ["user", "email"] },
                    "subject_id": { "type": "string" },
                    "permission": { "type": "string", "example": "admin" }
                  }
                }
              }
            }
          },
          "CreateProjectRequest": {
            "type": "object",
            "required": ["slug", "name"],
            "properties": {
              "slug": { "type": "string", "example": "demo-platform" },
              "name": { "type": "string", "example": "Demo Platform" },
              "description": { "type": ["string", "null"] }
            }
          },
          "UpdateProjectRequest": {
            "type": "object",
            "required": ["slug", "name", "active"],
            "properties": {
              "slug": { "type": "string", "example": "demo-platform" },
              "name": { "type": "string", "example": "Demo Platform" },
              "description": { "type": ["string", "null"] },
              "active": { "type": "boolean", "example": true }
            }
          },
          "UpsertProjectGitLabIntegrationRequest": {
            "type": "object",
            "required": ["gitlab_base_url", "gitlab_api_base_url", "gitlab_project_id", "token", "webhook_secret", "verify_tls", "sync_enabled"],
            "properties": {
              "gitlab_base_url": { "type": "string", "example": "https://gitlab.example.com" },
              "gitlab_api_base_url": { "type": "string", "example": "https://gitlab.example.com/api/v4" },
              "gitlab_project_id": { "type": "integer", "example": 1001 },
              "token": { "type": "string" },
              "webhook_secret": { "type": "string" },
              "verify_tls": { "type": "boolean", "example": true },
              "sync_enabled": { "type": "boolean", "example": true }
            }
          },
          "ValidateProjectGitLabIntegrationRequest": {
            "type": "object",
            "required": ["gitlab_base_url", "gitlab_api_base_url", "gitlab_project_id", "verify_tls"],
            "properties": {
              "gitlab_base_url": { "type": "string", "example": "https://gitlab.example.com" },
              "gitlab_api_base_url": { "type": "string", "example": "https://gitlab.example.com/api/v4" },
              "gitlab_project_id": { "type": "integer", "example": 1001 },
              "token": { "type": ["string", "null"] },
              "verify_tls": { "type": "boolean", "example": true }
            }
          },
          "GitLabIntegrationValidationResponse": {
            "type": "object",
            "required": ["valid", "project_name", "web_url", "visibility"],
            "properties": {
              "valid": { "type": "boolean", "example": true },
              "project_name": { "type": "string", "example": "Demo Platform" },
              "web_url": { "type": "string", "example": "https://gitlab.example.com/demo/platform" },
              "visibility": { "type": "string", "example": "private" }
            }
          },
          "GitLabIssueImportResponse": {
            "type": "object",
            "required": ["imported_count", "created_count", "updated_count"],
            "properties": {
              "imported_count": { "type": "integer", "example": 24 },
              "created_count": { "type": "integer", "example": 20 },
              "updated_count": { "type": "integer", "example": 4 }
            }
          },
          "GitLabCommentImportResponse": {
            "type": "object",
            "required": ["imported_count", "created_count", "updated_count"],
            "properties": {
              "imported_count": { "type": "integer", "example": 12 },
              "created_count": { "type": "integer", "example": 10 },
              "updated_count": { "type": "integer", "example": 2 }
            }
          },
          "GitLabWebhookResponse": {
            "type": "object",
            "required": ["status", "event_type", "handled"],
            "properties": {
              "status": { "type": "string", "example": "queued" },
              "event_type": { "type": "string", "example": "Issue Hook" },
              "handled": { "type": "boolean", "example": true },
              "job_id": { "type": ["string", "null"], "format": "uuid" },
              "issue_id": { "type": ["string", "null"], "format": "uuid" },
              "issue_iid": { "type": ["integer", "null"], "example": 42 }
            }
          },
          "CreateProjectIssueRequest": {
            "type": "object",
            "required": ["title"],
            "properties": {
              "title": { "type": "string", "example": "Bridge-created issue" },
              "description": { "type": ["string", "null"], "example": "Created through IssueHub" }
            }
          },
          "UpdateIssueRequest": {
            "type": "object",
            "properties": {
              "title": { "type": ["string", "null"], "example": "Updated issue title" },
              "description": { "type": ["string", "null"], "example": "Updated issue description" },
              "state": { "type": ["string", "null"], "enum": ["open", "closed", null], "example": "closed" }
            }
          },
          "CreateIssueCommentRequest": {
            "type": "object",
            "required": ["body"],
            "properties": {
              "body": { "type": "string", "example": "Pushed to GitLab through IssueHub" },
              "reply_to_note_id": { "type": ["integer", "null"], "example": 9001 }
            }
          },
          "Issue": {
            "type": "object",
            "required": ["id", "project_id", "project_slug", "project_name", "title", "description", "state", "sync_state", "gitlab_issue_iid", "version", "capabilities"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "project_id": { "type": "string", "format": "uuid" },
              "project_slug": { "type": "string" },
              "project_name": { "type": "string" },
              "title": { "type": "string" },
              "description": { "type": "string" },
              "state": { "type": "string", "example": "open" },
              "sync_state": { "type": "string", "example": "idle" },
              "gitlab_issue_iid": { "type": "integer", "example": 1 },
              "version": { "type": "integer", "example": 1 },
              "last_activity_at": { "type": ["string", "null"], "format": "date-time" },
              "capabilities": { "$ref": "#/components/schemas/IssueCapabilities" }
            }
          },
          "IssueCapabilities": {
            "type": "object",
            "required": ["can_view", "can_comment", "can_edit", "can_change_state", "can_manage_access", "can_sync_comments"],
            "properties": {
              "can_view": { "type": "boolean" },
              "can_comment": { "type": "boolean" },
              "can_edit": { "type": "boolean" },
              "can_change_state": { "type": "boolean" },
              "can_manage_access": { "type": "boolean" },
              "can_sync_comments": { "type": "boolean" }
            }
          },
          "IssueUpload": {
            "type": "object",
            "required": ["upload_id", "filename", "content_type", "byte_size", "proxy_path", "markdown"],
            "properties": {
              "upload_id": { "type": "string", "format": "uuid" },
              "filename": { "type": "string" },
              "content_type": { "type": "string" },
              "byte_size": { "type": "integer" },
              "proxy_path": { "type": "string" },
              "markdown": { "type": "string" }
            }
          },
          "IssueAccessAssignment": {
            "type": "object",
            "required": ["user_id", "email", "full_name", "permission"],
            "properties": {
              "user_id": { "type": "string", "format": "uuid" },
              "email": { "type": "string", "format": "email" },
              "full_name": { "type": "string" },
              "permission": { "type": "string", "example": "edit" }
            }
          },
          "IssueAccessUserOption": {
            "type": "object",
            "required": ["id", "email", "full_name"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "email": { "type": "string", "format": "email" },
              "full_name": { "type": "string" }
            }
          },
          "IssueAccessOverview": {
            "type": "object",
            "required": ["assignments", "available_users"],
            "properties": {
              "assignments": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/IssueAccessAssignment" }
              },
              "available_users": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/IssueAccessUserOption" }
              }
            }
          },
          "UpdateIssueAccessRequest": {
            "type": "object",
            "required": ["assignments"],
            "properties": {
              "assignments": {
                "type": "array",
                "items": {
                  "type": "object",
                  "required": ["user_id", "permission"],
                  "properties": {
                    "user_id": { "type": "string", "format": "uuid" },
                    "permission": { "type": "string", "example": "comment" }
                  }
                }
              }
            }
          },
          "Attachment": {
            "type": "object",
            "required": ["id", "filename", "content_type", "byte_size", "external_url", "proxy_path", "inline", "sync_state"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "filename": { "type": "string" },
              "content_type": { "type": "string", "example": "image/png" },
              "byte_size": { "type": "integer", "example": 245760 },
              "external_url": { "type": "string" },
              "proxy_path": { "type": "string" },
              "inline": { "type": "boolean" },
              "sync_state": { "type": "string", "example": "idle" }
            }
          },
          "Comment": {
            "type": "object",
            "required": ["id", "gitlab_note_id", "author_external_id", "author_name", "body_raw", "system_note", "sync_state", "attachments", "created_at"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "gitlab_note_id": { "type": "integer", "example": 9001 },
              "discussion_id": { "type": ["string", "null"] },
              "individual_note": { "type": "boolean" },
              "reply_to_gitlab_note_id": { "type": ["integer", "null"] },
              "author_external_id": { "type": "string" },
              "author_name": { "type": "string" },
              "body_raw": { "type": "string" },
              "system_note": { "type": "boolean" },
              "sync_state": { "type": "string" },
              "attachments": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/Attachment" }
              },
              "created_at": { "type": "string", "format": "date-time" }
            }
          },
          "IssueDetail": {
            "type": "object",
            "required": ["issue", "comments", "issue_attachments"],
            "properties": {
              "issue": { "$ref": "#/components/schemas/Issue" },
              "comments": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/Comment" }
              },
              "issue_attachments": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/Attachment" }
              }
            }
          },
          "EnqueueJobRequest": {
            "type": "object",
            "required": ["topic", "payload"],
            "properties": {
              "topic": { "type": "string", "example": "issue.sync.pull" },
              "payload": { "type": "object", "additionalProperties": true },
              "dedupe_key": { "type": ["string", "null"] }
            }
          },
          "JobResponse": {
            "type": "object",
            "required": ["id", "status"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "status": { "type": "string", "example": "pending" }
            }
          },
          "Job": {
            "type": "object",
            "required": ["id", "topic", "status", "attempt_count", "available_at", "created_at", "updated_at"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "topic": { "type": "string", "example": "gitlab.webhook.received" },
              "status": { "type": "string", "example": "pending" },
              "attempt_count": { "type": "integer", "example": 1 },
              "locked_by": { "type": ["string", "null"] },
              "dedupe_key": { "type": ["string", "null"] },
              "last_error": { "type": ["string", "null"] },
              "available_at": { "type": "string", "format": "date-time" },
              "created_at": { "type": "string", "format": "date-time" },
              "updated_at": { "type": "string", "format": "date-time" }
            }
          },
        }
      }
    }))
}

fn project_id_parameter() -> Value {
    json!({
      "name": "project_id",
      "in": "path",
      "required": true,
      "schema": { "type": "string", "format": "uuid" }
    })
}

fn issue_id_parameter() -> Value {
    json!({
      "name": "issue_id",
      "in": "path",
      "required": true,
      "schema": { "type": "string", "format": "uuid" }
    })
}

fn uuid_parameter(name: &str) -> Value {
    json!({
      "name": name,
      "in": "path",
      "required": true,
      "schema": { "type": "string", "format": "uuid" }
    })
}

fn token_parameter(name: &str) -> Value {
    json!({
      "name": name,
      "in": "path",
      "required": true,
      "schema": { "type": "string" }
    })
}
