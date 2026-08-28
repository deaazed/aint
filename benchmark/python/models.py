"""Pydantic request/response models - free validation and JSON
(de)serialization AINT's `json` stdlib module (milestone 25) had to
approximate by hand (flat-object `json_get`/`json_object` over
strings, no nesting, no arrays)."""

from enum import Enum

from pydantic import BaseModel, EmailStr


class Sentiment(str, Enum):
    positive = "positive"
    neutral = "neutral"
    negative = "negative"


class RegisterRequest(BaseModel):
    email: EmailStr
    password: str


class RegisterResponse(BaseModel):
    user_id: str


class LoginRequest(BaseModel):
    email: EmailStr
    password: str


class LoginResponse(BaseModel):
    token: str


class CreateTicketRequest(BaseModel):
    token: str
    subject: str
    body: str


class CreateTicketResponse(BaseModel):
    ticket_id: str
    priority: str


class Ticket(BaseModel):
    id: str
    user_id: str
    subject: str
    body: str
    status: str
    priority: str


class TicketListResponse(BaseModel):
    tickets: list[Ticket]


class ResolveTicketRequest(BaseModel):
    token: str
    ticket_id: str


class ResolveTicketResponse(BaseModel):
    status: str


class ErrorResponse(BaseModel):
    error: str
