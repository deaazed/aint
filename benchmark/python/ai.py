"""The AI-driven priority decision: classify a ticket's sentiment via
an LLM, and for a negative one, look up the customer's account tier
(a `tool` call, in AINT's vocabulary) before deciding priority.

This is the Python/LangGraph equivalent of AINT's
`infer classify_sentiment` + `tool database_get_account_tier` +
`priority_for` in ../../examples/customer_support/server.an and
priority_logic_test.an.
"""

from typing import Literal, TypedDict

from langgraph.graph import StateGraph, END
from openai import OpenAI
from pydantic import BaseModel

from models import Sentiment


class SentimentResult(BaseModel):
    sentiment: Sentiment


class GraphState(TypedDict):
    ticket_body: str
    user_id: str
    sentiment: str | None
    tier: str | None
    priority: str | None


# --- swappable model/tool implementations --------------------------------
#
# AINT's `Model`/`MockTool` traits (milestones 08/11) exist specifically
# so AI-touching code is testable without a live backend. The Python
# equivalent is the same idea with no framework support for it: a
# plain, hand-written indirection layer, swapped in tests via
# monkeypatching module-level functions.


def real_classify_sentiment(ticket_body: str) -> Sentiment:
    """Calls a real OpenAI-compatible endpoint for structured-output
    sentiment classification - needs OPENAI_API_KEY set, the same way
    AINT's own `infer` needs AINT_MODEL_URL set (see
    docs/milestones/25-real-application/SPEC.md)."""
    client = OpenAI()
    completion = client.chat.completions.parse(
        model="gpt-4o-mini",
        messages=[
            {
                "role": "user",
                "content": f"Classify the sentiment of this support ticket: {ticket_body}",
            }
        ],
        response_format=SentimentResult,
    )
    return completion.choices[0].message.parsed.sentiment


def real_lookup_account_tier(user_id: str) -> str:
    """Stands in for a real account-tier lookup (a database call, a
    billing service, ...) - a placeholder exactly the way AINT's own
    `tool` declarations are signature-only with no body."""
    raise NotImplementedError("no real account-tier backend is wired up")


classify_sentiment = real_classify_sentiment
lookup_account_tier = real_lookup_account_tier


# --- the graph -------------------------------------------------------------


def classify_node(state: GraphState) -> GraphState:
    sentiment = classify_sentiment(state["ticket_body"])
    return {**state, "sentiment": sentiment.value}


def lookup_tier_node(state: GraphState) -> GraphState:
    tier = lookup_account_tier(state["user_id"])
    return {**state, "tier": tier}


def decide_priority_node(state: GraphState) -> GraphState:
    if state["sentiment"] == Sentiment.negative.value and state.get("tier") == "premium":
        priority = "high"
    else:
        priority = "normal"
    return {**state, "priority": priority}


def route_after_classify(state: GraphState) -> Literal["lookup_tier", "decide_priority"]:
    if state["sentiment"] == Sentiment.negative.value:
        return "lookup_tier"
    return "decide_priority"


def build_graph():
    graph = StateGraph(GraphState)
    graph.add_node("classify", classify_node)
    graph.add_node("lookup_tier", lookup_tier_node)
    graph.add_node("decide_priority", decide_priority_node)
    graph.set_entry_point("classify")
    graph.add_conditional_edges("classify", route_after_classify)
    graph.add_edge("lookup_tier", "decide_priority")
    graph.add_edge("decide_priority", END)
    return graph.compile()


_graph = None


def decide_priority(ticket_body: str, user_id: str) -> str:
    global _graph
    if _graph is None:
        _graph = build_graph()
    result = _graph.invoke({"ticket_body": ticket_body, "user_id": user_id, "sentiment": None, "tier": None, "priority": None})
    return result["priority"]
