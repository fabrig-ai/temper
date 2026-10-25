"""The proof must judge the first committed assistant, and all browser trouble."""
from copy import deepcopy
from pathlib import Path
import runpy
import unittest

PROOF = runpy.run_path(str(Path(__file__).resolve().parents[2] / 'welcome-chatgpt/checks/proof'))
ANSWER = 'Hello from the local fake model.'


def conversation(messages):
    return {'activity': {'rows': [
        {'event': {'owner': {'decoded': None}}},
        {'event': {'owner': {'decoded': {'title': ANSWER}}}},
        {'event': {'owner': {'decoded': {'turn': {'decoded': {'messages': messages}}}}}},
    ]}}


def message(speaker, text):
    return {'speaker': speaker, 'blocks': [{'text': {'text': text}}]}


class WelcomeProof(unittest.TestCase):
    def test_first_assistant_reply_passes(self):
        document = conversation([message('user', 'Hello'), message('assistant', ANSWER)])
        self.assertEqual(PROOF['reply'](document), (0, []))

    def test_title_and_person_turn_do_not_count(self):
        document = conversation([message('user', ANSWER)])
        status, records = PROOF['reply'](document)
        self.assertEqual(status, 1)
        self.assertEqual(records[0]['fail'], 'reply/missing')

    def test_later_matching_reply_cannot_hide_wrong_first_reply(self):
        document = conversation([message('assistant', 'Wrong'), message('assistant', ANSWER)])
        status, records = PROOF['reply'](document)
        self.assertEqual(status, 1)
        self.assertEqual(records[0]['fail'], 'reply/first-assistant')

    def test_missing_conversation_is_unreadable(self):
        document = deepcopy(conversation([]))
        del document['activity']['rows']
        with self.assertRaises(KeyError):
            PROOF['reply'](document)

    def test_empty_trouble_passes(self):
        self.assertEqual(PROOF['trouble']([]), (0, []))

    def test_sign_in_trouble_is_a_failure_too(self):
        status, records = PROOF['trouble'](['{"kind":"network","during":"sign-in"}'])
        self.assertEqual(status, 1)
        self.assertEqual(records[0]['fail'], 'browser/trouble')

    def test_bad_trouble_is_unreadable(self):
        with self.assertRaises(ValueError):
            PROOF['trouble'](['{}'])
