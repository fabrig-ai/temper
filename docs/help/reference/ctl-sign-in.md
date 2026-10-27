# temper ctl sign-in

Kind: reference

Sign in with the link the deployment printed.

## sign-in

```
sign-in FILE
  FILE (file): a file; required. The file holding the link, or - for stdin
Sign in with the link the deployment printed
Prints: Your party, as me prints it
--json: the record sign_in.
Key: none.
It writes the token and the engine's address into DIR. A link
signs in once, and expires; `the sign-in link was refused` (exit
1) asks for a fresh link. With every sign-in in use, it says when
the earliest ends and until when this link stays usable. It leaves
the link's file as it was.
```
