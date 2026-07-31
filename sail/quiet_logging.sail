/* =============================================================================
 * quiet_logging.sail — silent replacement for the model's logging.sail.
 *
 * The stock logging.sail prints EVERY memory read/write to stdout
 * unconditionally (log_memory_read/log_memory_write are called from
 * memi/bang_memi on each byte access), which is unusable for an in-process
 * oracle. This file is passed to sail INSTEAD of logging.sail and provides
 * no-op bodies for every logging hook declared in prelude.sail plus the
 * printState used by main.sail.
 * ===========================================================================*/

$ifndef __X86_LOGGING
$define __X86_LOGGING

val printState : unit -> unit
function printState () = ()

function log_gpr_write(idx) = ()

function log_rflags_write() = ()

function log_rip_write() = ()

function log_memory_read(addr, len, value) = ()

function log_memory_write(addr, len, value) = ()

$endif
