# temper ctl pending

Kind: reference

List the requests DIR keeps, without reaching the engine.

## pending

```
pending
List the requests DIR keeps, without reaching the engine
Prints: A line per kept request, `OP, party N, first sent T` or `OP, party N, not sent`; nothing when DIR keeps none
--json: the record pending.
Key: none.
It reads DIR's store alone: it takes no lock, signs in to nothing
and sends nothing. Records these tools cannot read are counted,
never listed: `N kept records these tools cannot read; a call that
sends drops them`, with exit 0. A store that cannot be read exits
1.
```
