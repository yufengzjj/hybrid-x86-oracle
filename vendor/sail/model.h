#pragma once

#include "sail.h"
#include "sail_config.h"
#include "rts.h"
#include "elf.h"
extern void (*sail_rts_set_coverage_file)(const char *);

namespace model {
// enum write_kind
enum zwrite_kind { zWrite_plain, zWrite_conditional, zWrite_release, zWrite_exclusive, zWrite_exclusive_release, zWrite_RISCV_release, zWrite_RISCV_strong_release, zWrite_RISCV_conditional, zWrite_RISCV_conditional_release, zWrite_RISCV_conditional_strong_release, zWrite_X86_locked };

// struct vex_prefixes
struct zvex_prefixes {uint64_t zbits;};

// struct vex3_byte2
struct zvex3_byte2 {uint64_t zbits;};

// struct vex3_byte1
struct zvex3_byte1 {uint64_t zbits;};

// struct vex2_byte1
struct zvex2_byte1 {uint64_t zbits;};

// enum trans_kind
enum ztrans_kind { zTransaction_start, zTransaction_commit, zTransaction_abort };

// struct system_segment_descriptorbits
struct zsystem_segment_descriptorbits {lbits zbits;};

// struct system_segment_descriptor_attributesbits
struct zsystem_segment_descriptor_attributesbits {uint64_t zbits;};

// struct struct_wz64
struct zstruct_wzz64 {sail_int zregtype;};

// struct struct_wz32
struct zstruct_wzz32 {sail_int zregtype;};

// struct struct_wz128
struct zstruct_wzz128 {sail_int zregtype;};

// struct struct_paging_entry_no_page_fault_p
struct zstruct_paging_entry_no_page_fault_p {sail_int zsupervisor_mode_access_type;};

// struct sib
struct zsib {uint64_t zbits;};

// struct segment_selectorbits
struct zsegment_selectorbits {uint64_t zbits;};

// type abbreviation seg_reg_idx
typedef int64_t zseg_reg_idx;

// type abbreviation sbits
typedef lbits zsbits;

// struct rflagsbits
struct zrflagsbits {uint64_t zbits;};

// type abbreviation regtype
typedef uint64_t zregtype;

// enum read_kind
enum zread_kind { zRead_plain, zRead_reserve, zRead_acquire, zRead_exclusive, zRead_exclusive_acquire, zRead_stream, zRead_ifetch, zRead_RISCV_acquire, zRead_RISCV_strong_acquire, zRead_RISCV_reserved, zRead_RISCV_reserved_acquire, zRead_RISCV_reserved_strong_acquire, zRead_X86_locked };

// enum proc_mode
enum zproc_mode { zMode_64bit, zMode_Compatibility, zMode_Protected, zMode_Real, zMode_SMM };

// struct prefixes
struct zprefixes {uint64_t zbits;};

// union option<i>
enum kind_zoptionzIizK { Kind_zNonezIizK, Kind_zSomezIizK };

struct zoptionzIizK {
  enum kind_zoptionzIizK kind;
  union {
    struct { unit zNonezIizK; };
    struct { sail_int zSomezIizK; };
  } variants;
};

// union option<Rprefixes>
enum kind_zoptionzIRprefixeszK { Kind_zNonezIRprefixeszK, Kind_zSomezIRprefixeszK };

struct zoptionzIRprefixeszK {
  enum kind_zoptionzIRprefixeszK kind;
  union {
    struct { unit zNonezIRprefixeszK; };
    struct { struct zprefixes zSomezIRprefixeszK; };
  } variants;
};

// type abbreviation moffset_size
typedef int64_t zmoffset_sizze;

// struct modr_m
struct zmodr_m {uint64_t zbits;};

// struct ia32e_pte_4k_pagebits
struct zia32e_pte_4k_pagebits {uint64_t zbits;};

// struct ia32e_pml4ebits
struct zia32e_pml4ebits {uint64_t zbits;};

// struct ia32e_pdpte_pg_dirbits
struct zia32e_pdpte_pg_dirbits {uint64_t zbits;};

// struct ia32e_pdpte_1gb_pagebits
struct zia32e_pdpte_1gb_pagebits {uint64_t zbits;};

// struct ia32e_pde_pg_tablebits
struct zia32e_pde_pg_tablebits {uint64_t zbits;};

// struct ia32e_pde_2mb_pagebits
struct zia32e_pde_2mb_pagebits {uint64_t zbits;};

// struct ia32e_page_tablesbits
struct zia32e_page_tablesbits {uint64_t zbits;};

// struct ia32_eferbits
struct zia32_eferbits {uint64_t zbits;};

// struct gdtr_idtrbits
struct zgdtr_idtrbits {lbits zbits;};

// union exception
enum kind_zexception { Kind_zEmsg, Kind_zSyscall };

struct zexception {
  enum kind_zexception kind;
  union {
    struct { sail_string zEmsg; };
    struct { unit zSyscall; };
  } variants;
};

// struct evex_prefixes
struct zevex_prefixes {uint64_t zbits;};

// struct evex_byte3
struct zevex_byte3 {uint64_t zbits;};

// struct evex_byte2
struct zevex_byte2 {uint64_t zbits;};

// struct evex_byte1
struct zevex_byte1 {uint64_t zbits;};

// struct data_segment_descriptor_attributesbits
struct zdata_segment_descriptor_attributesbits {uint64_t zbits;};

// struct cr4bits
struct zcr4bits {uint64_t zbits;};

// struct cr3bits
struct zcr3bits {uint64_t zbits;};

// struct cr0bits
struct zcr0bits {uint64_t zbits;};

// struct code_segment_descriptorbits
struct zcode_segment_descriptorbits {uint64_t zbits;};

// struct code_segment_descriptor_attributesbits
struct zcode_segment_descriptor_attributesbits {uint64_t zbits;};

// struct call_gate_descriptorbits
struct zcall_gate_descriptorbits {lbits zbits;};

// enum cache_op_kind
enum zcache_op_kind { zCache_op_D_IVAC, zCache_op_D_ISW, zCache_op_D_CSW, zCache_op_D_CISW, zCache_op_D_ZVA, zCache_op_D_CVAC, zCache_op_D_CVAU, zCache_op_D_CIVAC, zCache_op_I_IALLUIS, zCache_op_I_IALLU, zCache_op_I_IVAU };

// type abbreviation base_reg_idx
typedef int64_t zbase_reg_idx;

// type abbreviation address_size
typedef int64_t zaddress_sizze;

// enum a64_barrier_type
enum za64_barrier_type { zA64_barrier_all, zA64_barrier_LD, zA64_barrier_ST };

// enum a64_barrier_domain
enum za64_barrier_domain { zA64_FullShare, zA64_InnerShare, zA64_OuterShare, zA64_NonShare };

struct zz5vecz8z5bv64z9 {
  size_t len;
  uint64_t *data;
};
typedef struct zz5vecz8z5bv64z9 zz5vecz8z5bv64z9;

struct zz5vecz8z5bv32z9 {
  size_t len;
  uint64_t *data;
};
typedef struct zz5vecz8z5bv32z9 zz5vecz8z5bv32z9;

struct zz5vecz8z5bv16z9 {
  size_t len;
  uint64_t *data;
};
typedef struct zz5vecz8z5bv16z9 zz5vecz8z5bv16z9;

struct zz5vecz8z5bvz9 {
  size_t len;
  lbits *data;
};
typedef struct zz5vecz8z5bvz9 zz5vecz8z5bvz9;

struct node_zz5listz8z5stringz9 {
  unsigned int rc;
  sail_string hd;
  struct node_zz5listz8z5stringz9 *tl;
};
typedef struct node_zz5listz8z5stringz9 *zz5listz8z5stringz9;

// struct tuple_(%i, %bv64, %i64)
struct ztuple_z8z5izCz0z5bv64zCz0z5i64z9 {
  sail_int ztup0;
  uint64_t ztup1;
  int64_t ztup2;
};

// struct tuple_(%bv64, %bv64, %i64)
struct ztuple_z8z5bv64zCz0z5bv64zCz0z5i64z9 {
  uint64_t ztup0;
  uint64_t ztup1;
  int64_t ztup2;
};

// struct tuple_(%bv64, %i64)
struct ztuple_z8z5bv64zCz0z5i64z9 {
  uint64_t ztup0;
  int64_t ztup1;
};

// struct tuple_(%bv16, %i64)
struct ztuple_z8z5bv16zCz0z5i64z9 {
  uint64_t ztup0;
  int64_t ztup1;
};

// struct tuple_(%struct zprefixes, %bv8, %bool)
struct ztuple_z8z5structz0zzprefixeszCz0z5bv8zCz0z5boolz9 {
  struct zprefixes ztup0;
  uint64_t ztup1;
  bool ztup2;
};

// struct tuple_(%bool, %i64)
struct ztuple_z8z5boolzCz0z5i64z9 {
  bool ztup0;
  int64_t ztup1;
};

// struct tuple_(%i, %bv48)
struct ztuple_z8z5izCz0z5bv48z9 {
  sail_int ztup0;
  uint64_t ztup1;
};

// struct tuple_(%string, (%i, %bv48))
struct ztuple_z8z5stringzCz0z8z5izCz0z5bv48z9z9 {
  sail_string ztup0;
  struct ztuple_z8z5izCz0z5bv48z9 ztup1;
};

// struct tuple_((%string, (%i, %bv48)), %bool)
struct ztuple_z8z8z5stringzCz0z8z5izCz0z5bv48z9z9zCz0z5boolz9 {
  struct ztuple_z8z5stringzCz0z8z5izCz0z5bv48z9z9 ztup0;
  bool ztup1;
};

// struct tuple_(%bool, %struct zia32e_page_tablesbits)
struct ztuple_z8z5boolzCz0z5structz0zzia32e_page_tablesbitsz9 {
  bool ztup0;
  struct zia32e_page_tablesbits ztup1;
};

// struct tuple_(%bv64, %i, %bv32)
struct ztuple_z8z5bv64zCz0z5izCz0z5bv32z9 {
  uint64_t ztup0;
  sail_int ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bv64, %i64, %bv32)
struct ztuple_z8z5bv64zCz0z5i64zCz0z5bv32z9 {
  uint64_t ztup0;
  int64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%string, %bv64)
struct ztuple_z8z5stringzCz0z5bv64z9 {
  sail_string ztup0;
  uint64_t ztup1;
};

// struct tuple_(%bool, (%string, %bv64))
struct ztuple_z8z5boolzCz0z8z5stringzCz0z5bv64z9z9 {
  bool ztup0;
  struct ztuple_z8z5stringzCz0z5bv64z9 ztup1;
};

// struct tuple_(%string, %bv)
struct ztuple_z8z5stringzCz0z5bvz9 {
  sail_string ztup0;
  lbits ztup1;
};

// struct tuple_(%bool, (%string, %bv))
struct ztuple_z8z5boolzCz0z8z5stringzCz0z5bvz9z9 {
  bool ztup0;
  struct ztuple_z8z5stringzCz0z5bvz9 ztup1;
};

// struct tuple_(%bv8, %struct zrflagsbits, %struct zrflagsbits)
struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 {
  uint64_t ztup0;
  struct zrflagsbits ztup1;
  struct zrflagsbits ztup2;
};

// struct tuple_(%bv16, %struct zrflagsbits, %struct zrflagsbits)
struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 {
  uint64_t ztup0;
  struct zrflagsbits ztup1;
  struct zrflagsbits ztup2;
};

// struct tuple_(%bv32, %struct zrflagsbits, %struct zrflagsbits)
struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 {
  uint64_t ztup0;
  struct zrflagsbits ztup1;
  struct zrflagsbits ztup2;
};

// struct tuple_(%bv64, %struct zrflagsbits, %struct zrflagsbits)
struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 {
  uint64_t ztup0;
  struct zrflagsbits ztup1;
  struct zrflagsbits ztup2;
};

// struct tuple_(%i64, %bv64, %i64)
struct ztuple_z8z5i64zCz0z5bv64zCz0z5i64z9 {
  int64_t ztup0;
  uint64_t ztup1;
  int64_t ztup2;
};

// struct tuple_(%bv, %i64, %bv64)
struct ztuple_z8z5bvzCz0z5i64zCz0z5bv64z9 {
  lbits ztup0;
  int64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%i64, %bv64)
struct ztuple_z8z5i64zCz0z5bv64z9 {
  int64_t ztup0;
  uint64_t ztup1;
};

// struct tuple_(%i, %bv)
struct ztuple_z8z5izCz0z5bvz9 {
  sail_int ztup0;
  lbits ztup1;
};

// struct tuple_(%bool, %bv16, %bv8)
struct ztuple_z8z5boolzCz0z5bv16zCz0z5bv8z9 {
  bool ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bool, %bv32, %bv16)
struct ztuple_z8z5boolzCz0z5bv32zCz0z5bv16z9 {
  bool ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bool, %bv64, %bv32)
struct ztuple_z8z5boolzCz0z5bv64zCz0z5bv32z9 {
  bool ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bool, %bv, %bv64)
struct ztuple_z8z5boolzCz0z5bvzCz0z5bv64z9 {
  bool ztup0;
  lbits ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bool, %bv8, %bv8)
struct ztuple_z8z5boolzCz0z5bv8zCz0z5bv8z9 {
  bool ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bool, %bv16, %bv16)
struct ztuple_z8z5boolzCz0z5bv16zCz0z5bv16z9 {
  bool ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bool, %bv32, %bv32)
struct ztuple_z8z5boolzCz0z5bv32zCz0z5bv32z9 {
  bool ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bool, %bv64, %bv64)
struct ztuple_z8z5boolzCz0z5bv64zCz0z5bv64z9 {
  bool ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bv64, %bv32)
struct ztuple_z8z5bv64zCz0z5bv32z9 {
  uint64_t ztup0;
  uint64_t ztup1;
};

// struct tuple_(%bv8, %bv8, %bv16)
struct ztuple_z8z5bv8zCz0z5bv8zCz0z5bv16z9 {
  uint64_t ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bv16, %bv16, %bv32)
struct ztuple_z8z5bv16zCz0z5bv16zCz0z5bv32z9 {
  uint64_t ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bv32, %bv32, %bv64)
struct ztuple_z8z5bv32zCz0z5bv32zCz0z5bv64z9 {
  uint64_t ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
};

// struct tuple_(%bv64, %bv64, %bv)
struct ztuple_z8z5bv64zCz0z5bv64zCz0z5bvz9 {
  uint64_t ztup0;
  uint64_t ztup1;
  lbits ztup2;
};

// struct tuple_(%bv8, %bv8, %bv16, %bv1)
struct ztuple_z8z5bv8zCz0z5bv8zCz0z5bv16zCz0z5bv1z9 {
  uint64_t ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
  uint64_t ztup3;
};

// struct tuple_(%bv16, %bv16, %bv32, %bv1)
struct ztuple_z8z5bv16zCz0z5bv16zCz0z5bv32zCz0z5bv1z9 {
  uint64_t ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
  uint64_t ztup3;
};

// struct tuple_(%bv32, %bv32, %bv64, %bv1)
struct ztuple_z8z5bv32zCz0z5bv32zCz0z5bv64zCz0z5bv1z9 {
  uint64_t ztup0;
  uint64_t ztup1;
  uint64_t ztup2;
  uint64_t ztup3;
};

// struct tuple_(%bv64, %bv64, %bv, %bv1)
struct ztuple_z8z5bv64zCz0z5bv64zCz0z5bvzCz0z5bv1z9 {
  uint64_t ztup0;
  uint64_t ztup1;
  lbits ztup2;
  uint64_t ztup3;
};

// struct tuple_(%struct zrflagsbits, %struct zrflagsbits)
struct ztuple_z8z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 {
  struct zrflagsbits ztup0;
  struct zrflagsbits ztup1;
};

// struct tuple_(%bv16, %bool, %struct zrflagsbits, %struct zrflagsbits)
struct ztuple_z8z5bv16zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 {
  uint64_t ztup0;
  bool ztup1;
  struct zrflagsbits ztup2;
  struct zrflagsbits ztup3;
};

// struct tuple_(%bv32, %bool, %struct zrflagsbits, %struct zrflagsbits)
struct ztuple_z8z5bv32zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 {
  uint64_t ztup0;
  bool ztup1;
  struct zrflagsbits ztup2;
  struct zrflagsbits ztup3;
};

// struct tuple_(%bv64, %bool, %struct zrflagsbits, %struct zrflagsbits)
struct ztuple_z8z5bv64zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 {
  uint64_t ztup0;
  bool ztup1;
  struct zrflagsbits ztup2;
  struct zrflagsbits ztup3;
};

// struct tuple_(%i64, %i64)
struct ztuple_z8z5i64zCz0z5i64z9 {
  int64_t ztup0;
  int64_t ztup1;
};

// struct tuple_(%i, %i)
struct ztuple_z8z5izCz0z5iz9 {
  sail_int ztup0;
  sail_int ztup1;
};

struct zz5vecz8z5boolz9 {
  size_t len;
  bool *data;
};
typedef struct zz5vecz8z5boolz9 zz5vecz8z5boolz9;

struct zz5vecz8z5i64z9 {
  size_t len;
  int64_t *data;
};
typedef struct zz5vecz8z5i64z9 zz5vecz8z5i64z9;

// struct tuple_(%bool, %bool, %bool)
struct ztuple_z8z5boolzCz0z5boolzCz0z5boolz9 {
  bool ztup0;
  bool ztup1;
  bool ztup2;
};

// struct tuple_(%struct zprefixes, %bv8)
struct ztuple_z8z5structz0zzprefixeszCz0z5bv8z9 {
  struct zprefixes ztup0;
  uint64_t ztup1;
};

// struct tuple_(%string, %i)
struct ztuple_z8z5stringzCz0z5iz9 {
  sail_string ztup0;
  sail_int ztup1;
};

class Model {
public:

  bool zneq_int(sail_int, sail_int);

  void zsail_mask(lbits *rop, sail_int, lbits);

  void startup_zsail_mask(void);

  void finish_zsail_mask(void);

  bool zis_somezIizK(struct zoptionzIizK);

  void zfeature_flag_fn(sail_int *rop, const_sail_string);

  void zfeature_flags_fn(sail_int *rop, zz5listz8z5stringz9);

  uint64_t zx86_model_errorzIB64zK(const_sail_string);

  uint64_t zx86_model_errorzIB48zK(const_sail_string);

  uint64_t zx86_model_errorzIB32zK(const_sail_string);

  uint64_t zx86_model_errorzIB16zK(const_sail_string);

  void zx86_model_errorzIbzK(lbits *rop, const_sail_string);

  unit zx86_model_errorzIuzK(const_sail_string);

  void zx86_model_errorzIz8izCB64zCI64z9zK(struct ztuple_z8z5izCz0z5bv64zCz0z5i64z9 *rop, const_sail_string);

  struct ztuple_z8z5bv64zCz0z5bv64zCz0z5i64z9 zx86_model_errorzIz8B64zCB64zCI64z9zK(const_sail_string);

  struct ztuple_z8z5bv64zCz0z5i64z9 zx86_model_errorzIz8B64zCI64z9zK(const_sail_string);

  struct ztuple_z8z5bv16zCz0z5i64z9 zx86_model_errorzIz8B16zCI64z9zK(const_sail_string);

  void zx86_faultzIbzK(lbits *rop, const_sail_string);

  unit zx86_faultzIuzK(const_sail_string);

  bool zeq_bits_nat(lbits, sail_int);

  void startup_zeq_bits_nat(void);

  void finish_zeq_bits_nat(void);

  uint64_t zbool_to_bits(bool);

  bool zbits_to_bool(uint64_t);

  void zsail_mask_signed(lbits *rop, sail_int, lbits);

  void startup_zsail_mask_signed(void);

  void finish_zsail_mask_signed(void);

  void ztrunc(lbits *rop, int64_t, lbits);

  void startup_ztrunc(void);

  void finish_ztrunc(void);

  void ztrunc_signed(lbits *rop, int64_t, lbits);

  void startup_ztrunc_signed(void);

  void finish_ztrunc_signed(void);

  void zbitslice(lbits *rop, lbits, sail_int, sail_int);

  void startup_zbitslice(void);

  void finish_zbitslice(void);

  void zsigned_bitslice(lbits *rop, lbits, sail_int, sail_int);

  void startup_zsigned_bitslice(void);

  void finish_zsigned_bitslice(void);

  void zcheck_range(sail_int *rop, sail_int, sail_int, sail_int);

  void zbits_of_int(lbits *rop, sail_int, sail_int);

  void startup_zbits_of_int(void);

  void finish_zbits_of_int(void);

  bool zfits_in_signed_bitvector(sail_int, sail_int);

  void startup_zfits_in_signed_bitvector(void);

  void finish_zfits_in_signed_bitvector(void);

  bool zisEven(sail_int);

  void startup_zisEven(void);

  void finish_zisEven(void);

  void zb_xor(sail_int *rop, sail_int, sail_int);

  void startup_zb_xor(void);

  void finish_zb_xor(void);

  bool zin_list(const_sail_string, zz5listz8z5stringz9);

  bool zunsigned_byte_p_int(sail_int, sail_int);

  void startup_zunsigned_byte_p_int(void);

  void finish_zunsigned_byte_p_int(void);

  bool zsigned_byte_p_int(sail_int, sail_int);

  void startup_zsigned_byte_p_int(void);

  void finish_zsigned_byte_p_int(void);

  void zloghead_int(sail_int *rop, sail_int, sail_int);

  void startup_zloghead_int(void);

  void finish_zloghead_int(void);

  void zloghead_bits(lbits *rop, sail_int, lbits);

  bool zlogbitp_int(sail_int, sail_int);

  void startup_zlogbitp_int(void);

  void finish_zlogbitp_int(void);

  bool zlogbitp_bits(sail_int, lbits);

  void startup_zlogbitp_bits(void);

  void finish_zlogbitp_bits(void);

  uint64_t zlogbit_bits(sail_int, lbits);

  void startup_zlogbit_bits(void);

  void finish_zlogbit_bits(void);

  void zlognot_int(sail_int *rop, sail_int);

  void startup_zlognot_int(void);

  void finish_zlognot_int(void);

  void zlogcount_int(sail_int *rop, sail_int);

  void zfloor2(sail_int *rop, sail_int, sail_int);

  void startup_zfloor2(void);

  void finish_zfloor2(void);

  void zlogand_int(sail_int *rop, sail_int, sail_int);

  void zlogior_int(sail_int *rop, sail_int, sail_int);

  void startup_zlogior_int(void);

  void finish_zlogior_int(void);

  void zlogxor(lbits *rop, lbits, lbits);

  void zbinary_logapp(sail_int *rop, sail_int, sail_int, sail_int);

  void startup_zbinary_logapp(void);

  void finish_zbinary_logapp(void);

  void zbinary_logext(sail_int *rop, sail_int, sail_int);

  void startup_zbinary_logext(void);

  void finish_zbinary_logext(void);

  void zash_left_int(sail_int *rop, sail_int, sail_int);

  void startup_zash_left_int(void);

  void finish_zash_left_int(void);

  void zash_int(sail_int *rop, sail_int, sail_int);

  void startup_zash_int(void);

  void finish_zash_int(void);

  void zabs(sail_int *rop, sail_int);

  void startup_zabs(void);

  void finish_zabs(void);

  void zmod(sail_int *rop, sail_int, sail_int);

  void startup_zmod(void);

  void finish_zmod(void);

  void zrotate_left_bits(lbits *rop, lbits, sail_int, sail_int);

  void startup_zrotate_left_bits(void);

  void finish_zrotate_left_bits(void);

  void zrotate_right_bits(lbits *rop, lbits, sail_int, sail_int);

  void startup_zrotate_right_bits(void);

  void finish_zrotate_right_bits(void);

  void zchangeSlice_int(sail_int *rop, sail_int, sail_int, sail_int, sail_int);

  void startup_zchangeSlice_int(void);

  void finish_zchangeSlice_int(void);

  void zchangeSlice_bits(lbits *rop, lbits, sail_int, sail_int, lbits);

  void startup_zchangeSlice_bits(void);

  void finish_zchangeSlice_bits(void);

  unit zlog_memory_read(lbits, sail_int, lbits);

  uint64_t zmemi(lbits);

  void startup_zmemi(void);

  void finish_zmemi(void);

  unit zlog_memory_write(lbits, sail_int, lbits);

  unit zbang_memi(lbits, uint64_t);

  void startup_zbang_memi(void);

  void finish_zbang_memi(void);

  unit z__SetConfig(const_sail_string, sail_int);

  unit zlog_gpr_write(int64_t);

  bool zms(unit);

  bool zfault(unit);

  uint64_t zia32e_la_to_pa(uint64_t, const_sail_string);

  bool zcanonical_address_p(sail_int);

  bool zn64_bit_modep(unit);

  unit zobserve_mem_write(uint64_t, sail_int);

  void zrb(lbits *rop, sail_int, lbits, const_sail_string);

  void startup_zrb(void);

  void finish_zrb(void);

  unit zwb(sail_int, lbits, const_sail_string, lbits);

  void startup_zwb(void);

  void finish_zwb(void);

  uint64_t zregval_from_reg(uint64_t);

  uint64_t zregval_into_reg(uint64_t);

  uint64_t z_get_vex_prefixes_byte0(struct zvex_prefixes);

  struct zvex_prefixes z_update_vex_prefixes_byte0(struct zvex_prefixes, uint64_t);

  void startup_z_update_vex_prefixes_byte0(void);

  void finish_z_update_vex_prefixes_byte0(void);

  uint64_t z_get_vex_prefixes_byte1(struct zvex_prefixes);

  struct zvex_prefixes z_update_vex_prefixes_byte1(struct zvex_prefixes, uint64_t);

  void startup_z_update_vex_prefixes_byte1(void);

  void finish_z_update_vex_prefixes_byte1(void);

  uint64_t z_get_vex_prefixes_byte2(struct zvex_prefixes);

  struct zvex_prefixes z_update_vex_prefixes_byte2(struct zvex_prefixes, uint64_t);

  void startup_z_update_vex_prefixes_byte2(void);

  void finish_z_update_vex_prefixes_byte2(void);

  struct zvex2_byte1 zMk_vex2_byte1(uint64_t);

  void startup_zMk_vex2_byte1(void);

  void finish_zMk_vex2_byte1(void);

  uint64_t z_get_vex2_byte1_l(struct zvex2_byte1);

  uint64_t z_get_vex2_byte1_pp(struct zvex2_byte1);

  uint64_t z_get_vex2_byte1_vvvv(struct zvex2_byte1);

  struct zvex3_byte1 zMk_vex3_byte1(uint64_t);

  void startup_zMk_vex3_byte1(void);

  void finish_zMk_vex3_byte1(void);

  uint64_t z_get_vex3_byte1_m_mmmm(struct zvex3_byte1);

  struct zvex3_byte2 zMk_vex3_byte2(uint64_t);

  void startup_zMk_vex3_byte2(void);

  void finish_zMk_vex3_byte2(void);

  uint64_t z_get_vex3_byte2_l(struct zvex3_byte2);

  uint64_t z_get_vex3_byte2_pp(struct zvex3_byte2);

  uint64_t z_get_vex3_byte2_vvvv(struct zvex3_byte2);

  uint64_t z_get_vex3_byte2_w(struct zvex3_byte2);

  bool zvex_prefixes_map_p(uint64_t, struct zvex_prefixes);

  void startup_zvex_prefixes_map_p(void);

  void finish_zvex_prefixes_map_p(void);

  uint64_t zvex_get_vvvv(struct zvex_prefixes);

  void startup_zvex_get_vvvv(void);

  void finish_zvex_get_vvvv(void);

  uint64_t zvex_get_l(struct zvex_prefixes);

  void startup_zvex_get_l(void);

  void finish_zvex_get_l(void);

  uint64_t zvex_get_pp(struct zvex_prefixes);

  void startup_zvex_get_pp(void);

  void finish_zvex_get_pp(void);

  uint64_t zvex_get_w(struct zvex_prefixes);

  void startup_zvex_get_w(void);

  void finish_zvex_get_w(void);

  struct zevex_prefixes z_update_evex_prefixes_byte0(struct zevex_prefixes, uint64_t);

  void startup_z_update_evex_prefixes_byte0(void);

  void finish_z_update_evex_prefixes_byte0(void);

  uint64_t z_get_evex_prefixes_byte1(struct zevex_prefixes);

  struct zevex_prefixes z_update_evex_prefixes_byte1(struct zevex_prefixes, uint64_t);

  void startup_z_update_evex_prefixes_byte1(void);

  void finish_z_update_evex_prefixes_byte1(void);

  uint64_t z_get_evex_prefixes_byte2(struct zevex_prefixes);

  struct zevex_prefixes z_update_evex_prefixes_byte2(struct zevex_prefixes, uint64_t);

  void startup_z_update_evex_prefixes_byte2(void);

  void finish_z_update_evex_prefixes_byte2(void);

  uint64_t z_get_evex_prefixes_byte3(struct zevex_prefixes);

  struct zevex_byte1 zMk_evex_byte1(uint64_t);

  void startup_zMk_evex_byte1(void);

  void finish_zMk_evex_byte1(void);

  uint64_t z_get_evex_byte1_mm(struct zevex_byte1);

  uint64_t z_get_evex_byte1_res(struct zevex_byte1);

  struct zevex_byte2 zMk_evex_byte2(uint64_t);

  void startup_zMk_evex_byte2(void);

  void finish_zMk_evex_byte2(void);

  uint64_t z_get_evex_byte2_pp(struct zevex_byte2);

  uint64_t z_get_evex_byte2_res(struct zevex_byte2);

  uint64_t z_get_evex_byte2_vvvv(struct zevex_byte2);

  uint64_t z_get_evex_byte2_w(struct zevex_byte2);

  struct zevex_byte3 zMk_evex_byte3(uint64_t);

  void startup_zMk_evex_byte3(void);

  void finish_zMk_evex_byte3(void);

  uint64_t z_get_evex_byte3_v_prime(struct zevex_byte3);

  uint64_t z_get_evex_byte3_vl_rc(struct zevex_byte3);

  uint64_t zevex_get_vvvv(struct zevex_prefixes);

  void startup_zevex_get_vvvv(void);

  void finish_zevex_get_vvvv(void);

  uint64_t zevex_get_v_prime(struct zevex_prefixes);

  void startup_zevex_get_v_prime(void);

  void finish_zevex_get_v_prime(void);

  uint64_t zevex_get_vl_rc(struct zevex_prefixes);

  void startup_zevex_get_vl_rc(void);

  void finish_zevex_get_vl_rc(void);

  uint64_t zevex_get_pp(struct zevex_prefixes);

  void startup_zevex_get_pp(void);

  void finish_zevex_get_pp(void);

  uint64_t zevex_get_w(struct zevex_prefixes);

  void startup_zevex_get_w(void);

  void finish_zevex_get_w(void);

  struct zmodr_m zMk_modr_m(uint64_t);

  void startup_zMk_modr_m(void);

  void finish_zMk_modr_m(void);

  uint64_t z_get_modr_m_mod(struct zmodr_m);

  uint64_t z_get_modr_m_r_m(struct zmodr_m);

  uint64_t z_get_modr_m_reg(struct zmodr_m);

  struct zsib zMk_sib(uint64_t);

  void startup_zMk_sib(void);

  void finish_zMk_sib(void);

  uint64_t z_get_sib_base(struct zsib);

  uint64_t z_get_sib_index(struct zsib);

  uint64_t z_get_sib_scale(struct zsib);

  struct zrflagsbits zundefined_rflagsbits(unit);

  void startup_zundefined_rflagsbits(void);

  void finish_zundefined_rflagsbits(void);

  struct zrflagsbits zMk_rflagsbits(uint64_t);

  void startup_zMk_rflagsbits(void);

  void finish_zMk_rflagsbits(void);

  uint64_t z_get_rflagsbits_ac(struct zrflagsbits);

  uint64_t z_get_rflagsbits_af(struct zrflagsbits);

  struct zrflagsbits z_update_rflagsbits_af(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_af(void);

  void finish_z_update_rflagsbits_af(void);

  uint64_t z_get_rflagsbits_cf(struct zrflagsbits);

  struct zrflagsbits z_update_rflagsbits_cf(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_cf(void);

  void finish_z_update_rflagsbits_cf(void);

  uint64_t z_get_rflagsbits_df(struct zrflagsbits);

  struct zrflagsbits z_update_rflagsbits_df(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_df(void);

  void finish_z_update_rflagsbits_df(void);

  uint64_t z_get_rflagsbits_of(struct zrflagsbits);

  struct zrflagsbits z_update_rflagsbits_of(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_of(void);

  void finish_z_update_rflagsbits_of(void);

  uint64_t z_get_rflagsbits_pf(struct zrflagsbits);

  struct zrflagsbits z_update_rflagsbits_pf(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_pf(void);

  void finish_z_update_rflagsbits_pf(void);

  struct zrflagsbits z_update_rflagsbits_res1(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_res1(void);

  void finish_z_update_rflagsbits_res1(void);

  struct zrflagsbits z_update_rflagsbits_res2(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_res2(void);

  void finish_z_update_rflagsbits_res2(void);

  struct zrflagsbits z_update_rflagsbits_res3(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_res3(void);

  void finish_z_update_rflagsbits_res3(void);

  uint64_t z_get_rflagsbits_rf(struct zrflagsbits);

  struct zrflagsbits z_update_rflagsbits_rf(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_rf(void);

  void finish_z_update_rflagsbits_rf(void);

  uint64_t z_get_rflagsbits_sf(struct zrflagsbits);

  struct zrflagsbits z_update_rflagsbits_sf(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_sf(void);

  void finish_z_update_rflagsbits_sf(void);

  struct zrflagsbits z_update_rflagsbits_vif(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_vif(void);

  void finish_z_update_rflagsbits_vif(void);

  struct zrflagsbits z_update_rflagsbits_vip(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_vip(void);

  void finish_z_update_rflagsbits_vip(void);

  uint64_t z_get_rflagsbits_vm(struct zrflagsbits);

  struct zrflagsbits z_update_rflagsbits_vm(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_vm(void);

  void finish_z_update_rflagsbits_vm(void);

  uint64_t z_get_rflagsbits_zzf(struct zrflagsbits);

  struct zrflagsbits z_update_rflagsbits_zzf(struct zrflagsbits, uint64_t);

  void startup_z_update_rflagsbits_zzf(void);

  void finish_z_update_rflagsbits_zzf(void);

  struct zcr0bits zMk_cr0bits(uint64_t);

  void startup_zMk_cr0bits(void);

  void finish_zMk_cr0bits(void);

  uint64_t z_get_cr0bits_am(struct zcr0bits);

  uint64_t z_get_cr0bits_em(struct zcr0bits);

  uint64_t z_get_cr0bits_pe(struct zcr0bits);

  uint64_t z_get_cr0bits_ts(struct zcr0bits);

  uint64_t z_get_cr0bits_wp(struct zcr0bits);

  struct zcr3bits zMk_cr3bits(uint64_t);

  void startup_zMk_cr3bits(void);

  void finish_zMk_cr3bits(void);

  uint64_t z_get_cr3bits_pdb(struct zcr3bits);

  struct zcr4bits zMk_cr4bits(uint64_t);

  void startup_zMk_cr4bits(void);

  void finish_zMk_cr4bits(void);

  uint64_t z_get_cr4bits_de(struct zcr4bits);

  uint64_t z_get_cr4bits_osfxsr(struct zcr4bits);

  uint64_t z_get_cr4bits_osxsave(struct zcr4bits);

  uint64_t z_get_cr4bits_pce(struct zcr4bits);

  uint64_t z_get_cr4bits_smap(struct zcr4bits);

  uint64_t z_get_cr4bits_smep(struct zcr4bits);

  uint64_t z_get_cr4bits_umip(struct zcr4bits);

  struct zia32_eferbits zMk_ia32_eferbits(uint64_t);

  void startup_zMk_ia32_eferbits(void);

  void finish_zMk_ia32_eferbits(void);

  uint64_t z_get_ia32_eferbits_lma(struct zia32_eferbits);

  struct zia32_eferbits z_update_ia32_eferbits_lma(struct zia32_eferbits, uint64_t);

  void startup_z_update_ia32_eferbits_lma(void);

  void finish_z_update_ia32_eferbits_lma(void);

  uint64_t z_get_ia32_eferbits_nxe(struct zia32_eferbits);

  uint64_t z_get_ia32_eferbits_sce(struct zia32_eferbits);

  uint64_t zread_gpr(int64_t);

  unit zwrite_gpr(int64_t, uint64_t);

  uint64_t zrgfi(sail_int);

  void startup_zrgfi(void);

  void finish_zrgfi(void);

  unit zwrite_rgfi(sail_int, uint64_t);

  void startup_zwrite_rgfi(void);

  void finish_zwrite_rgfi(void);

  uint64_t zread_rip(unit);

  void startup_zread_rip(void);

  void finish_zread_rip(void);

  unit zwrite_rip(uint64_t);

  uint64_t zread_msr(int64_t);

  unit zwrite_msr(int64_t, uint64_t);

  void startup_zwrite_msr(void);

  void finish_zwrite_msr(void);

  bool zin_64bit_mode(enum zproc_mode);

  bool zin_compatibility_mode(enum zproc_mode);

  bool zin_protected_mode(enum zproc_mode);

  enum zproc_mode zx86_operation_mode(unit);

  struct zprefixes zMk_prefixes(uint64_t);

  void startup_zMk_prefixes(void);

  void finish_zMk_prefixes(void);

  uint64_t z_get_prefixes_adr(struct zprefixes);

  struct zprefixes z_update_prefixes_adr(struct zprefixes, uint64_t);

  void startup_z_update_prefixes_adr(void);

  void finish_z_update_prefixes_adr(void);

  uint64_t z_get_prefixes_lck(struct zprefixes);

  struct zprefixes z_update_prefixes_lck(struct zprefixes, uint64_t);

  void startup_z_update_prefixes_lck(void);

  void finish_z_update_prefixes_lck(void);

  uint64_t z_get_prefixes_num(struct zprefixes);

  struct zprefixes z_update_prefixes_num(struct zprefixes, uint64_t);

  void startup_z_update_prefixes_num(void);

  void finish_z_update_prefixes_num(void);

  uint64_t z_get_prefixes_nxt(struct zprefixes);

  struct zprefixes z_update_prefixes_nxt(struct zprefixes, uint64_t);

  void startup_z_update_prefixes_nxt(void);

  void finish_z_update_prefixes_nxt(void);

  uint64_t z_get_prefixes_opr(struct zprefixes);

  struct zprefixes z_update_prefixes_opr(struct zprefixes, uint64_t);

  void startup_z_update_prefixes_opr(void);

  void finish_z_update_prefixes_opr(void);

  uint64_t z_get_prefixes_rep(struct zprefixes);

  struct zprefixes z_update_prefixes_rep(struct zprefixes, uint64_t);

  void startup_z_update_prefixes_rep(void);

  void finish_z_update_prefixes_rep(void);

  uint64_t z_get_prefixes_seg(struct zprefixes);

  struct zprefixes z_update_prefixes_seg(struct zprefixes, uint64_t);

  void startup_z_update_prefixes_seg(void);

  void finish_z_update_prefixes_seg(void);

  bool zis_ext_prefix_byte(enum zproc_mode, struct zprefixes, uint64_t, int64_t, uint64_t);

  struct ztuple_z8z5structz0zzprefixeszCz0z5bv8zCz0z5boolz9 zprocess_ext_prefix_byte(enum zproc_mode, struct zprefixes, uint64_t, int64_t, uint64_t);

  bool zext_one_byte_opcode_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  bool zext_two_byte_opcode_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  bool zext_vex_0f_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zvex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  bool zext_vex_0f38_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zvex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  bool zext_vex_0f3a_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zvex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  bool zext_evex_0f_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zevex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  bool zext_evex_0f38_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zevex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  bool zext_evex_0f3a_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zevex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  unit zunimplemented_x86_syscall_app_view(enum zproc_mode, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t);

  struct zsegment_selectorbits zMk_segment_selectorbits(uint64_t);

  void startup_zMk_segment_selectorbits(void);

  void finish_zMk_segment_selectorbits(void);

  uint64_t z_get_segment_selectorbits_index(struct zsegment_selectorbits);

  uint64_t z_get_segment_selectorbits_rpl(struct zsegment_selectorbits);

  struct zsegment_selectorbits z_update_segment_selectorbits_rpl(struct zsegment_selectorbits, uint64_t);

  void startup_z_update_segment_selectorbits_rpl(void);

  void finish_z_update_segment_selectorbits_rpl(void);

  uint64_t z_get_segment_selectorbits_ti(struct zsegment_selectorbits);

  void zMk_gdtr_idtrbits(struct zgdtr_idtrbits *rop, lbits);

  void startup_zMk_gdtr_idtrbits(void);

  void finish_zMk_gdtr_idtrbits(void);

  uint64_t z_get_gdtr_idtrbits_base_addr(struct zgdtr_idtrbits);

  void startup_z_get_gdtr_idtrbits_base_addr(void);

  void finish_z_get_gdtr_idtrbits_base_addr(void);

  void z_update_gdtr_idtrbits_base_addr(struct zgdtr_idtrbits *rop, struct zgdtr_idtrbits, uint64_t);

  void startup_z_update_gdtr_idtrbits_base_addr(void);

  void finish_z_update_gdtr_idtrbits_base_addr(void);

  uint64_t z_get_gdtr_idtrbits_limit(struct zgdtr_idtrbits);

  void startup_z_get_gdtr_idtrbits_limit(void);

  void finish_z_get_gdtr_idtrbits_limit(void);

  void z_update_gdtr_idtrbits_limit(struct zgdtr_idtrbits *rop, struct zgdtr_idtrbits, uint64_t);

  void startup_z_update_gdtr_idtrbits_limit(void);

  void finish_z_update_gdtr_idtrbits_limit(void);

  struct zcode_segment_descriptorbits zMk_code_segment_descriptorbits(uint64_t);

  void startup_zMk_code_segment_descriptorbits(void);

  void finish_zMk_code_segment_descriptorbits(void);

  uint64_t z_get_code_segment_descriptorbits_a(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_avl(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_c(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_d(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_dpl(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_g(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_l(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_limit15_0(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_limit19_16(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_msb_of_type(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_p(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_r(struct zcode_segment_descriptorbits);

  uint64_t z_get_code_segment_descriptorbits_s(struct zcode_segment_descriptorbits);

  struct zcode_segment_descriptor_attributesbits zMk_code_segment_descriptor_attributesbits(uint64_t);

  void startup_zMk_code_segment_descriptor_attributesbits(void);

  void finish_zMk_code_segment_descriptor_attributesbits(void);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_a(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_a(void);

  void finish_z_update_code_segment_descriptor_attributesbits_a(void);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_avl(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_avl(void);

  void finish_z_update_code_segment_descriptor_attributesbits_avl(void);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_c(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_c(void);

  void finish_z_update_code_segment_descriptor_attributesbits_c(void);

  uint64_t z_get_code_segment_descriptor_attributesbits_d(struct zcode_segment_descriptor_attributesbits);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_d(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_d(void);

  void finish_z_update_code_segment_descriptor_attributesbits_d(void);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_dpl(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_dpl(void);

  void finish_z_update_code_segment_descriptor_attributesbits_dpl(void);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_g(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_g(void);

  void finish_z_update_code_segment_descriptor_attributesbits_g(void);

  uint64_t z_get_code_segment_descriptor_attributesbits_l(struct zcode_segment_descriptor_attributesbits);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_l(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_l(void);

  void finish_z_update_code_segment_descriptor_attributesbits_l(void);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_msb_of_type(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_msb_of_type(void);

  void finish_z_update_code_segment_descriptor_attributesbits_msb_of_type(void);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_p(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_p(void);

  void finish_z_update_code_segment_descriptor_attributesbits_p(void);

  uint64_t z_get_code_segment_descriptor_attributesbits_r(struct zcode_segment_descriptor_attributesbits);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_r(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_r(void);

  void finish_z_update_code_segment_descriptor_attributesbits_r(void);

  struct zcode_segment_descriptor_attributesbits z_update_code_segment_descriptor_attributesbits_s(struct zcode_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_code_segment_descriptor_attributesbits_s(void);

  void finish_z_update_code_segment_descriptor_attributesbits_s(void);

  struct zdata_segment_descriptor_attributesbits zMk_data_segment_descriptor_attributesbits(uint64_t);

  void startup_zMk_data_segment_descriptor_attributesbits(void);

  void finish_zMk_data_segment_descriptor_attributesbits(void);

  struct zdata_segment_descriptor_attributesbits z_update_data_segment_descriptor_attributesbits_a(struct zdata_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_data_segment_descriptor_attributesbits_a(void);

  void finish_z_update_data_segment_descriptor_attributesbits_a(void);

  uint64_t z_get_data_segment_descriptor_attributesbits_d_b(struct zdata_segment_descriptor_attributesbits);

  struct zdata_segment_descriptor_attributesbits z_update_data_segment_descriptor_attributesbits_d_b(struct zdata_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_data_segment_descriptor_attributesbits_d_b(void);

  void finish_z_update_data_segment_descriptor_attributesbits_d_b(void);

  struct zdata_segment_descriptor_attributesbits z_update_data_segment_descriptor_attributesbits_dpl(struct zdata_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_data_segment_descriptor_attributesbits_dpl(void);

  void finish_z_update_data_segment_descriptor_attributesbits_dpl(void);

  uint64_t z_get_data_segment_descriptor_attributesbits_e(struct zdata_segment_descriptor_attributesbits);

  struct zdata_segment_descriptor_attributesbits z_update_data_segment_descriptor_attributesbits_e(struct zdata_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_data_segment_descriptor_attributesbits_e(void);

  void finish_z_update_data_segment_descriptor_attributesbits_e(void);

  struct zdata_segment_descriptor_attributesbits z_update_data_segment_descriptor_attributesbits_g(struct zdata_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_data_segment_descriptor_attributesbits_g(void);

  void finish_z_update_data_segment_descriptor_attributesbits_g(void);

  struct zdata_segment_descriptor_attributesbits z_update_data_segment_descriptor_attributesbits_msb_of_type(struct zdata_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_data_segment_descriptor_attributesbits_msb_of_type(void);

  void finish_z_update_data_segment_descriptor_attributesbits_msb_of_type(void);

  struct zdata_segment_descriptor_attributesbits z_update_data_segment_descriptor_attributesbits_p(struct zdata_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_data_segment_descriptor_attributesbits_p(void);

  void finish_z_update_data_segment_descriptor_attributesbits_p(void);

  struct zdata_segment_descriptor_attributesbits z_update_data_segment_descriptor_attributesbits_s(struct zdata_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_data_segment_descriptor_attributesbits_s(void);

  void finish_z_update_data_segment_descriptor_attributesbits_s(void);

  uint64_t z_get_data_segment_descriptor_attributesbits_w(struct zdata_segment_descriptor_attributesbits);

  struct zdata_segment_descriptor_attributesbits z_update_data_segment_descriptor_attributesbits_w(struct zdata_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_data_segment_descriptor_attributesbits_w(void);

  void finish_z_update_data_segment_descriptor_attributesbits_w(void);

  void zMk_system_segment_descriptorbits(struct zsystem_segment_descriptorbits *rop, lbits);

  void startup_zMk_system_segment_descriptorbits(void);

  void finish_zMk_system_segment_descriptorbits(void);

  uint64_t z_get_system_segment_descriptorbits_all_zzeroeszL(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_all_zzeroeszL(void);

  void finish_z_get_system_segment_descriptorbits_all_zzeroeszL(void);

  uint64_t z_get_system_segment_descriptorbits_avl(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_avl(void);

  void finish_z_get_system_segment_descriptorbits_avl(void);

  uint64_t z_get_system_segment_descriptorbits_base15_0(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_base15_0(void);

  void finish_z_get_system_segment_descriptorbits_base15_0(void);

  uint64_t z_get_system_segment_descriptorbits_base23_16(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_base23_16(void);

  void finish_z_get_system_segment_descriptorbits_base23_16(void);

  uint64_t z_get_system_segment_descriptorbits_base31_24(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_base31_24(void);

  void finish_z_get_system_segment_descriptorbits_base31_24(void);

  uint64_t z_get_system_segment_descriptorbits_base63_32(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_base63_32(void);

  void finish_z_get_system_segment_descriptorbits_base63_32(void);

  uint64_t z_get_system_segment_descriptorbits_dpl(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_dpl(void);

  void finish_z_get_system_segment_descriptorbits_dpl(void);

  uint64_t z_get_system_segment_descriptorbits_g(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_g(void);

  void finish_z_get_system_segment_descriptorbits_g(void);

  uint64_t z_get_system_segment_descriptorbits_limit15_0(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_limit15_0(void);

  void finish_z_get_system_segment_descriptorbits_limit15_0(void);

  uint64_t z_get_system_segment_descriptorbits_limit19_16(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_limit19_16(void);

  void finish_z_get_system_segment_descriptorbits_limit19_16(void);

  uint64_t z_get_system_segment_descriptorbits_p(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_p(void);

  void finish_z_get_system_segment_descriptorbits_p(void);

  uint64_t z_get_system_segment_descriptorbits_s(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_s(void);

  void finish_z_get_system_segment_descriptorbits_s(void);

  uint64_t z_get_system_segment_descriptorbits_sailtype(struct zsystem_segment_descriptorbits);

  void startup_z_get_system_segment_descriptorbits_sailtype(void);

  void finish_z_get_system_segment_descriptorbits_sailtype(void);

  struct zsystem_segment_descriptor_attributesbits z_update_system_segment_descriptor_attributesbits_avl(struct zsystem_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_system_segment_descriptor_attributesbits_avl(void);

  void finish_z_update_system_segment_descriptor_attributesbits_avl(void);

  struct zsystem_segment_descriptor_attributesbits z_update_system_segment_descriptor_attributesbits_dpl(struct zsystem_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_system_segment_descriptor_attributesbits_dpl(void);

  void finish_z_update_system_segment_descriptor_attributesbits_dpl(void);

  struct zsystem_segment_descriptor_attributesbits z_update_system_segment_descriptor_attributesbits_g(struct zsystem_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_system_segment_descriptor_attributesbits_g(void);

  void finish_z_update_system_segment_descriptor_attributesbits_g(void);

  struct zsystem_segment_descriptor_attributesbits z_update_system_segment_descriptor_attributesbits_p(struct zsystem_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_system_segment_descriptor_attributesbits_p(void);

  void finish_z_update_system_segment_descriptor_attributesbits_p(void);

  struct zsystem_segment_descriptor_attributesbits z_update_system_segment_descriptor_attributesbits_s(struct zsystem_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_system_segment_descriptor_attributesbits_s(void);

  void finish_z_update_system_segment_descriptor_attributesbits_s(void);

  struct zsystem_segment_descriptor_attributesbits z_update_system_segment_descriptor_attributesbits_sailtype(struct zsystem_segment_descriptor_attributesbits, uint64_t);

  void startup_z_update_system_segment_descriptor_attributesbits_sailtype(void);

  void finish_z_update_system_segment_descriptor_attributesbits_sailtype(void);

  void zMk_call_gate_descriptorbits(struct zcall_gate_descriptorbits *rop, lbits);

  void startup_zMk_call_gate_descriptorbits(void);

  void finish_zMk_call_gate_descriptorbits(void);

  uint64_t z_get_call_gate_descriptorbits_all_zzeroeszL(struct zcall_gate_descriptorbits);

  void startup_z_get_call_gate_descriptorbits_all_zzeroeszL(void);

  void finish_z_get_call_gate_descriptorbits_all_zzeroeszL(void);

  uint64_t z_get_call_gate_descriptorbits_dpl(struct zcall_gate_descriptorbits);

  void startup_z_get_call_gate_descriptorbits_dpl(void);

  void finish_z_get_call_gate_descriptorbits_dpl(void);

  uint64_t z_get_call_gate_descriptorbits_offset15_0(struct zcall_gate_descriptorbits);

  void startup_z_get_call_gate_descriptorbits_offset15_0(void);

  void finish_z_get_call_gate_descriptorbits_offset15_0(void);

  uint64_t z_get_call_gate_descriptorbits_offset31_16(struct zcall_gate_descriptorbits);

  void startup_z_get_call_gate_descriptorbits_offset31_16(void);

  void finish_z_get_call_gate_descriptorbits_offset31_16(void);

  uint64_t z_get_call_gate_descriptorbits_offset63_32(struct zcall_gate_descriptorbits);

  void startup_z_get_call_gate_descriptorbits_offset63_32(void);

  void finish_z_get_call_gate_descriptorbits_offset63_32(void);

  uint64_t z_get_call_gate_descriptorbits_p(struct zcall_gate_descriptorbits);

  void startup_z_get_call_gate_descriptorbits_p(void);

  void finish_z_get_call_gate_descriptorbits_p(void);

  uint64_t z_get_call_gate_descriptorbits_s(struct zcall_gate_descriptorbits);

  void startup_z_get_call_gate_descriptorbits_s(void);

  void finish_z_get_call_gate_descriptorbits_s(void);

  uint64_t z_get_call_gate_descriptorbits_sailtype(struct zcall_gate_descriptorbits);

  void startup_z_get_call_gate_descriptorbits_sailtype(void);

  void finish_z_get_call_gate_descriptorbits_sailtype(void);

  uint64_t z_get_call_gate_descriptorbits_selector(struct zcall_gate_descriptorbits);

  void startup_z_get_call_gate_descriptorbits_selector(void);

  void finish_z_get_call_gate_descriptorbits_selector(void);

  struct zia32e_page_tablesbits zMk_ia32e_page_tablesbits(uint64_t);

  void startup_zMk_ia32e_page_tablesbits(void);

  void finish_zMk_ia32e_page_tablesbits(void);

  uint64_t z_get_ia32e_page_tablesbits_a(struct zia32e_page_tablesbits);

  struct zia32e_page_tablesbits z_update_ia32e_page_tablesbits_a(struct zia32e_page_tablesbits, uint64_t);

  void startup_z_update_ia32e_page_tablesbits_a(void);

  void finish_z_update_ia32e_page_tablesbits_a(void);

  uint64_t z_get_ia32e_page_tablesbits_d(struct zia32e_page_tablesbits);

  struct zia32e_page_tablesbits z_update_ia32e_page_tablesbits_d(struct zia32e_page_tablesbits, uint64_t);

  void startup_z_update_ia32e_page_tablesbits_d(void);

  void finish_z_update_ia32e_page_tablesbits_d(void);

  uint64_t z_get_ia32e_page_tablesbits_p(struct zia32e_page_tablesbits);

  uint64_t z_get_ia32e_page_tablesbits_ps(struct zia32e_page_tablesbits);

  uint64_t z_get_ia32e_page_tablesbits_r_w(struct zia32e_page_tablesbits);

  uint64_t z_get_ia32e_page_tablesbits_u_s(struct zia32e_page_tablesbits);

  uint64_t z_get_ia32e_page_tablesbits_xd(struct zia32e_page_tablesbits);

  struct zia32e_pml4ebits zMk_ia32e_pml4ebits(uint64_t);

  void startup_zMk_ia32e_pml4ebits(void);

  void finish_zMk_ia32e_pml4ebits(void);

  uint64_t z_get_ia32e_pml4ebits_pdpt(struct zia32e_pml4ebits);

  struct zia32e_pdpte_1gb_pagebits zMk_ia32e_pdpte_1gb_pagebits(uint64_t);

  void startup_zMk_ia32e_pdpte_1gb_pagebits(void);

  void finish_zMk_ia32e_pdpte_1gb_pagebits(void);

  uint64_t z_get_ia32e_pdpte_1gb_pagebits_page(struct zia32e_pdpte_1gb_pagebits);

  struct zia32e_pdpte_pg_dirbits zMk_ia32e_pdpte_pg_dirbits(uint64_t);

  void startup_zMk_ia32e_pdpte_pg_dirbits(void);

  void finish_zMk_ia32e_pdpte_pg_dirbits(void);

  uint64_t z_get_ia32e_pdpte_pg_dirbits_pd(struct zia32e_pdpte_pg_dirbits);

  struct zia32e_pde_2mb_pagebits zMk_ia32e_pde_2mb_pagebits(uint64_t);

  void startup_zMk_ia32e_pde_2mb_pagebits(void);

  void finish_zMk_ia32e_pde_2mb_pagebits(void);

  uint64_t z_get_ia32e_pde_2mb_pagebits_page(struct zia32e_pde_2mb_pagebits);

  struct zia32e_pde_pg_tablebits zMk_ia32e_pde_pg_tablebits(uint64_t);

  void startup_zMk_ia32e_pde_pg_tablebits(void);

  void finish_zMk_ia32e_pde_pg_tablebits(void);

  uint64_t z_get_ia32e_pde_pg_tablebits_pt(struct zia32e_pde_pg_tablebits);

  struct zia32e_pte_4k_pagebits zMk_ia32e_pte_4k_pagebits(uint64_t);

  void startup_zMk_ia32e_pte_4k_pagebits(void);

  void finish_zMk_ia32e_pte_4k_pagebits(void);

  uint64_t z_get_ia32e_pte_4k_pagebits_page(struct zia32e_pte_4k_pagebits);

  uint64_t zreg_index(uint64_t, uint64_t, uint64_t);

  void startup_zreg_index(void);

  void finish_zreg_index(void);

  uint64_t zrr08(uint64_t, uint64_t);

  void startup_zrr08(void);

  void finish_zrr08(void);

  unit zwr08(uint64_t, uint64_t, uint64_t);

  void startup_zwr08(void);

  void finish_zwr08(void);

  uint64_t zrr16(uint64_t);

  void startup_zrr16(void);

  void finish_zrr16(void);

  unit zwr16(uint64_t, uint64_t);

  void startup_zwr16(void);

  void finish_zwr16(void);

  uint64_t zrr32(uint64_t);

  void startup_zrr32(void);

  void finish_zrr32(void);

  unit zwr32(uint64_t, uint64_t);

  void startup_zwr32(void);

  void finish_zwr32(void);

  uint64_t zrr64(uint64_t);

  void startup_zrr64(void);

  void finish_zrr64(void);

  unit zwr64(uint64_t, uint64_t);

  void startup_zwr64(void);

  void finish_zwr64(void);

  uint64_t zrgfi_sizze(uint64_t, uint64_t, uint64_t);

  void startup_zrgfi_sizze(void);

  void finish_zrgfi_sizze(void);

  unit zwrite_rgfi_sizze(uint64_t, uint64_t, uint64_t, uint64_t);

  void startup_zwrite_rgfi_sizze(void);

  void finish_zwrite_rgfi_sizze(void);

  uint64_t zrzz32(uint64_t);

  void startup_zrzz32(void);

  void finish_zrzz32(void);

  uint64_t zrzz64(uint64_t);

  void startup_zrzz64(void);

  void finish_zrzz64(void);

  void zrzz128(lbits *rop, uint64_t);

  void startup_zrzz128(void);

  void finish_zrzz128(void);

  unit zwzz32(uint64_t, uint64_t, struct zstruct_wzz32);

  void startup_zwzz32(void);

  void finish_zwzz32(void);

  unit zwzz64(uint64_t, uint64_t, struct zstruct_wzz64);

  void startup_zwzz64(void);

  void finish_zwzz64(void);

  unit zwzz128(uint64_t, lbits, struct zstruct_wzz128);

  void startup_zwzz128(void);

  void finish_zwzz128(void);

  uint64_t zrx32(uint64_t);

  uint64_t zrx64(uint64_t);

  void zrx128(lbits *rop, uint64_t);

  unit zwx32(uint64_t, uint64_t);

  void startup_zwx32(void);

  void finish_zwx32(void);

  unit zwx64(uint64_t, uint64_t);

  void startup_zwx64(void);

  void finish_zwx64(void);

  unit zwx128(uint64_t, lbits);

  void startup_zwx128(void);

  void finish_zwx128(void);

  void zxmmi_sizze(lbits *rop, uint64_t, uint64_t);

  void startup_zxmmi_sizze(void);

  void finish_zxmmi_sizze(void);

  unit zwrite_xmmi_sizze(uint64_t, uint64_t, sail_int);

  void startup_zwrite_xmmi_sizze(void);

  void finish_zwrite_xmmi_sizze(void);

  unit zwrite_user_rflags(struct zrflagsbits, struct zrflagsbits);

  void startup_zwrite_user_rflags(void);

  void finish_zwrite_user_rflags(void);

  void startup_zn64_bit_modep(void);

  void finish_zn64_bit_modep(void);

  uint64_t zrm_low_32(sail_int);

  void startup_zrm_low_32(void);

  void finish_zrm_low_32(void);

  unit zwm_low_32(uint64_t, uint64_t);

  void startup_zwm_low_32(void);

  void finish_zwm_low_32(void);

  uint64_t zrm_low_64(sail_int);

  void startup_zrm_low_64(void);

  void finish_zrm_low_64(void);

  unit zwm_low_64(uint64_t, uint64_t);

  void startup_zwm_low_64(void);

  void finish_zwm_low_64(void);

  void zpage_fault_err_no(sail_int *rop, uint64_t, const_sail_string, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t);

  void startup_zpage_fault_err_no(void);

  void finish_zpage_fault_err_no(void);

  struct ztuple_z8z5boolzCz0z5i64z9 zpage_fault_exception(uint64_t, sail_int);

  uint64_t zpage_present(uint64_t);

  void startup_zpage_present(void);

  void finish_zpage_present(void);

  uint64_t zpage_sizze(uint64_t);

  void startup_zpage_sizze(void);

  void finish_zpage_sizze(void);

  uint64_t zpage_read_write(uint64_t);

  void startup_zpage_read_write(void);

  void finish_zpage_read_write(void);

  uint64_t zpage_user_supervisor(uint64_t);

  void startup_zpage_user_supervisor(void);

  void finish_zpage_user_supervisor(void);

  uint64_t zpage_execute_disable(uint64_t);

  void startup_zpage_execute_disable(void);

  void finish_zpage_execute_disable(void);

  uint64_t zaccessed_bit(uint64_t);

  void startup_zaccessed_bit(void);

  void finish_zaccessed_bit(void);

  uint64_t zdirty_bit(uint64_t);

  void startup_zdirty_bit(void);

  void finish_zdirty_bit(void);

  struct zia32e_page_tablesbits zset_accessed_bit(uint64_t);

  void startup_zset_accessed_bit(void);

  void finish_zset_accessed_bit(void);

  struct zia32e_page_tablesbits zset_dirty_bit(uint64_t);

  void startup_zset_dirty_bit(void);

  void finish_zset_dirty_bit(void);

  uint64_t zpage_table_entry_addr(uint64_t, uint64_t);

  void startup_zpage_table_entry_addr(void);

  void finish_zpage_table_entry_addr(void);

  uint64_t zpage_directory_entry_addr(uint64_t, uint64_t);

  void startup_zpage_directory_entry_addr(void);

  void finish_zpage_directory_entry_addr(void);

  uint64_t zpage_dir_ptr_table_entry_addr(uint64_t, uint64_t);

  void startup_zpage_dir_ptr_table_entry_addr(void);

  void finish_zpage_dir_ptr_table_entry_addr(void);

  uint64_t zpml4_table_entry_addr(uint64_t, uint64_t);

  void startup_zpml4_table_entry_addr(void);

  void finish_zpml4_table_entry_addr(void);

  struct ztuple_z8z5boolzCz0z5i64z9 zpaging_entry_no_page_fault_p(uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, const_sail_string, uint64_t, struct zstruct_paging_entry_no_page_fault_p);

  void startup_zpaging_entry_no_page_fault_p(void);

  void finish_zpaging_entry_no_page_fault_p(void);

  void zia32e_la_to_pa_page_table(sail_int *rop, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, const_sail_string, uint64_t);

  void startup_zia32e_la_to_pa_page_table(void);

  void finish_zia32e_la_to_pa_page_table(void);

  void zia32e_la_to_pa_page_directory(sail_int *rop, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, const_sail_string, uint64_t);

  void startup_zia32e_la_to_pa_page_directory(void);

  void finish_zia32e_la_to_pa_page_directory(void);

  void zia32e_la_to_pa_page_dir_ptr_table(sail_int *rop, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, const_sail_string, uint64_t);

  void startup_zia32e_la_to_pa_page_dir_ptr_table(void);

  void finish_zia32e_la_to_pa_page_dir_ptr_table(void);

  uint64_t zia32e_la_to_pa_pml4_table(uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, uint64_t, const_sail_string, uint64_t);

  void startup_zia32e_la_to_pa_pml4_table(void);

  void finish_zia32e_la_to_pa_pml4_table(void);

  void startup_zia32e_la_to_pa(void);

  void finish_zia32e_la_to_pa(void);

  uint64_t zla_to_pa(uint64_t, const_sail_string);

  void startup_zla_to_pa(void);

  void finish_zla_to_pa(void);

  uint64_t zrml08(uint64_t, const_sail_string);

  void startup_zrml08(void);

  void finish_zrml08(void);

  uint64_t zrml16(uint64_t, const_sail_string);

  void startup_zrml16(void);

  void finish_zrml16(void);

  uint64_t zrml32(uint64_t, const_sail_string);

  void startup_zrml32(void);

  void finish_zrml32(void);

  uint64_t zrml48(uint64_t, const_sail_string);

  void startup_zrml48(void);

  void finish_zrml48(void);

  uint64_t zrml64(uint64_t, const_sail_string);

  void startup_zrml64(void);

  void finish_zrml64(void);

  void zrml80(lbits *rop, uint64_t, const_sail_string);

  void startup_zrml80(void);

  void finish_zrml80(void);

  void zrml128(lbits *rop, uint64_t, const_sail_string);

  void startup_zrml128(void);

  void finish_zrml128(void);

  void zrml_sizze(lbits *rop, int64_t, uint64_t, const_sail_string);

  void startup_zrml_sizze(void);

  void finish_zrml_sizze(void);

  void zsegment_base_and_bounds(struct ztuple_z8z5bv64zCz0z5izCz0z5bv32z9 *rop, enum zproc_mode, int64_t);

  void startup_zsegment_base_and_bounds(void);

  void finish_zsegment_base_and_bounds(void);

  uint64_t zea_to_la(enum zproc_mode, uint64_t, int64_t, int64_t);

  void startup_zea_to_la(void);

  void finish_zea_to_la(void);

  void zia32e_valid_code_segment_descriptor_p(struct ztuple_z8z5boolzCz0z8z5stringzCz0z5bv64z9z9 *rop, uint64_t);

  void startup_zia32e_valid_code_segment_descriptor_p(void);

  void finish_zia32e_valid_code_segment_descriptor_p(void);

  void zia32e_valid_ldt_segment_descriptor_p(struct ztuple_z8z5boolzCz0z8z5stringzCz0z5bvz9z9 *rop, lbits);

  void startup_zia32e_valid_ldt_segment_descriptor_p(void);

  void finish_zia32e_valid_ldt_segment_descriptor_p(void);

  void zia32e_valid_call_gate_segment_descriptor_p(struct ztuple_z8z5boolzCz0z8z5stringzCz0z5bvz9z9 *rop, lbits);

  void startup_zia32e_valid_call_gate_segment_descriptor_p(void);

  void finish_zia32e_valid_call_gate_segment_descriptor_p(void);

  struct zcode_segment_descriptor_attributesbits zmake_code_segment_attr_field(uint64_t);

  void startup_zmake_code_segment_attr_field(void);

  void finish_zmake_code_segment_attr_field(void);

  struct zsystem_segment_descriptor_attributesbits zmake_system_segment_attr_field(lbits);

  void startup_zmake_system_segment_attr_field(void);

  void finish_zmake_system_segment_attr_field(void);

  bool zaddress_aligned_p(uint64_t, int64_t, bool);

  unit zcheck_linear_memory_access(enum zproc_mode, int64_t, int64_t, uint64_t, struct zoptionzIizK, int64_t, const_sail_string, bool, bool);

  void startup_zcheck_linear_memory_access(void);

  void finish_zcheck_linear_memory_access(void);

  void zselect_base_register(struct zoptionzIizK *rop, enum zproc_mode, uint64_t, uint64_t, uint64_t, struct zsib);

  int64_t zselect_address_sizze(enum zproc_mode, struct zoptionzIRprefixeszK);

  void startup_zselect_address_sizze(void);

  void finish_zselect_address_sizze(void);

  int64_t zselect_moffset_sizze(enum zproc_mode, struct zoptionzIRprefixeszK);

  int64_t zselect_segment_register(enum zproc_mode, struct zprefixes, uint64_t, uint64_t, struct zsib);

  void startup_zselect_segment_register(void);

  void finish_zselect_segment_register(void);

  void zload_bytes_from_ea(lbits *rop, enum zproc_mode, int64_t, int64_t, uint64_t, struct zoptionzIizK, int64_t, const_sail_string, bool, bool);

  void startup_zload_bytes_from_ea(void);

  void finish_zload_bytes_from_ea(void);

  unit zstore_bytes_to_ea(enum zproc_mode, int64_t, int64_t, uint64_t, struct zoptionzIizK, int64_t, lbits, bool, bool);

  void startup_zstore_bytes_to_ea(void);

  void finish_zstore_bytes_to_ea(void);

  unit z__ListConfig(unit);

  void startup_zcanonical_address_p(void);

  void finish_zcanonical_address_p(void);

  uint64_t zcf_spec8(uint64_t);

  void startup_zcf_spec8(void);

  void finish_zcf_spec8(void);

  uint64_t zcf_spec16(uint64_t);

  void startup_zcf_spec16(void);

  void finish_zcf_spec16(void);

  uint64_t zcf_spec32(uint64_t);

  void startup_zcf_spec32(void);

  void finish_zcf_spec32(void);

  uint64_t zcf_spec64(lbits);

  void startup_zcf_spec64(void);

  void finish_zcf_spec64(void);

  uint64_t zof_spec8(uint64_t);

  void startup_zof_spec8(void);

  void finish_zof_spec8(void);

  uint64_t zof_spec16(uint64_t);

  void startup_zof_spec16(void);

  void finish_zof_spec16(void);

  uint64_t zof_spec32(uint64_t);

  void startup_zof_spec32(void);

  void finish_zof_spec32(void);

  uint64_t zof_spec64(lbits);

  void startup_zof_spec64(void);

  void finish_zof_spec64(void);

  uint64_t zzzf_spec(sail_int);

  void startup_zzzf_spec(void);

  void finish_zzzf_spec(void);

  uint64_t zpf_spec8(uint64_t);

  void startup_zpf_spec8(void);

  void finish_zpf_spec8(void);

  uint64_t zpf_spec16(uint64_t);

  void startup_zpf_spec16(void);

  void finish_zpf_spec16(void);

  uint64_t zpf_spec32(uint64_t);

  void startup_zpf_spec32(void);

  void finish_zpf_spec32(void);

  uint64_t zpf_spec64(uint64_t);

  void startup_zpf_spec64(void);

  void finish_zpf_spec64(void);

  uint64_t zsf_spec8(uint64_t);

  uint64_t zsf_spec16(uint64_t);

  uint64_t zsf_spec32(uint64_t);

  uint64_t zsf_spec64(uint64_t);

  uint64_t zadd_af_spec8(uint64_t, uint64_t);

  void startup_zadd_af_spec8(void);

  void finish_zadd_af_spec8(void);

  uint64_t zadd_af_spec16(uint64_t, uint64_t);

  void startup_zadd_af_spec16(void);

  void finish_zadd_af_spec16(void);

  uint64_t zadd_af_spec32(uint64_t, uint64_t);

  void startup_zadd_af_spec32(void);

  void finish_zadd_af_spec32(void);

  uint64_t zadd_af_spec64(uint64_t, uint64_t);

  void startup_zadd_af_spec64(void);

  void finish_zadd_af_spec64(void);

  uint64_t zsub_af_spec8(uint64_t, uint64_t);

  void startup_zsub_af_spec8(void);

  void finish_zsub_af_spec8(void);

  uint64_t zsub_af_spec16(uint64_t, uint64_t);

  void startup_zsub_af_spec16(void);

  void finish_zsub_af_spec16(void);

  uint64_t zsub_af_spec32(uint64_t, uint64_t);

  void startup_zsub_af_spec32(void);

  void finish_zsub_af_spec32(void);

  uint64_t zsub_af_spec64(uint64_t, uint64_t);

  void startup_zsub_af_spec64(void);

  void finish_zsub_af_spec64(void);

  uint64_t zadc_af_spec8(uint64_t, uint64_t, uint64_t);

  void startup_zadc_af_spec8(void);

  void finish_zadc_af_spec8(void);

  uint64_t zadc_af_spec16(uint64_t, uint64_t, uint64_t);

  void startup_zadc_af_spec16(void);

  void finish_zadc_af_spec16(void);

  uint64_t zadc_af_spec32(uint64_t, uint64_t, uint64_t);

  void startup_zadc_af_spec32(void);

  void finish_zadc_af_spec32(void);

  uint64_t zadc_af_spec64(uint64_t, uint64_t, uint64_t);

  void startup_zadc_af_spec64(void);

  void finish_zadc_af_spec64(void);

  uint64_t zsbb_af_spec8(uint64_t, uint64_t, uint64_t);

  void startup_zsbb_af_spec8(void);

  void finish_zsbb_af_spec8(void);

  uint64_t zsbb_af_spec16(uint64_t, uint64_t, uint64_t);

  void startup_zsbb_af_spec16(void);

  void finish_zsbb_af_spec16(void);

  uint64_t zsbb_af_spec32(uint64_t, uint64_t, uint64_t);

  void startup_zsbb_af_spec32(void);

  void finish_zsbb_af_spec32(void);

  uint64_t zsbb_af_spec64(uint64_t, uint64_t, uint64_t);

  void startup_zsbb_af_spec64(void);

  void finish_zsbb_af_spec64(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_add_spec_1(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_add_spec_1(void);

  void finish_zgpr_add_spec_1(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_add_spec_2(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_add_spec_2(void);

  void finish_zgpr_add_spec_2(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_add_spec_4(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_add_spec_4(void);

  void finish_zgpr_add_spec_4(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_add_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_add_spec_8(void);

  void finish_zgpr_add_spec_8(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_adc_spec_1(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_adc_spec_1(void);

  void finish_zgpr_adc_spec_1(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_adc_spec_2(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_adc_spec_2(void);

  void finish_zgpr_adc_spec_2(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_adc_spec_4(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_adc_spec_4(void);

  void finish_zgpr_adc_spec_4(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_adc_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_adc_spec_8(void);

  void finish_zgpr_adc_spec_8(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_sub_spec_1(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_sub_spec_1(void);

  void finish_zgpr_sub_spec_1(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_sub_spec_2(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_sub_spec_2(void);

  void finish_zgpr_sub_spec_2(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_sub_spec_4(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_sub_spec_4(void);

  void finish_zgpr_sub_spec_4(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_sub_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_sub_spec_8(void);

  void finish_zgpr_sub_spec_8(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_sbb_spec_1(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_sbb_spec_1(void);

  void finish_zgpr_sbb_spec_1(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_sbb_spec_2(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_sbb_spec_2(void);

  void finish_zgpr_sbb_spec_2(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_sbb_spec_4(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_sbb_spec_4(void);

  void finish_zgpr_sbb_spec_4(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_sbb_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_sbb_spec_8(void);

  void finish_zgpr_sbb_spec_8(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_or_spec_1(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_or_spec_1(void);

  void finish_zgpr_or_spec_1(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_or_spec_2(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_or_spec_2(void);

  void finish_zgpr_or_spec_2(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_or_spec_4(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_or_spec_4(void);

  void finish_zgpr_or_spec_4(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_or_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_or_spec_8(void);

  void finish_zgpr_or_spec_8(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_and_spec_1(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_and_spec_1(void);

  void finish_zgpr_and_spec_1(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_and_spec_2(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_and_spec_2(void);

  void finish_zgpr_and_spec_2(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_and_spec_4(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_and_spec_4(void);

  void finish_zgpr_and_spec_4(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_and_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_and_spec_8(void);

  void finish_zgpr_and_spec_8(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_xor_spec_1(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_xor_spec_1(void);

  void finish_zgpr_xor_spec_1(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_xor_spec_2(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_xor_spec_2(void);

  void finish_zgpr_xor_spec_2(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_xor_spec_4(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_xor_spec_4(void);

  void finish_zgpr_xor_spec_4(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_xor_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_xor_spec_8(void);

  void finish_zgpr_xor_spec_8(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_arith_logic_spec_1(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_arith_logic_spec_1(void);

  void finish_zgpr_arith_logic_spec_1(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_arith_logic_spec_2(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_arith_logic_spec_2(void);

  void finish_zgpr_arith_logic_spec_2(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_arith_logic_spec_4(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_arith_logic_spec_4(void);

  void finish_zgpr_arith_logic_spec_4(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_arith_logic_spec_8(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_arith_logic_spec_8(void);

  void finish_zgpr_arith_logic_spec_8(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zgpr_arith_logic_spec(int64_t, int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zgpr_arith_logic_spec(void);

  void finish_zgpr_arith_logic_spec(void);

  void startup_zaddress_aligned_p(void);

  void finish_zaddress_aligned_p(void);

  uint64_t zrme08(enum zproc_mode, int64_t, uint64_t, struct zoptionzIizK, int64_t, const_sail_string);

  void startup_zrme08(void);

  void finish_zrme08(void);

  uint64_t zrme16(enum zproc_mode, int64_t, uint64_t, struct zoptionzIizK, int64_t, const_sail_string, bool);

  void startup_zrme16(void);

  void finish_zrme16(void);

  void zrme_sizze(lbits *rop, enum zproc_mode, int64_t, int64_t, uint64_t, struct zoptionzIizK, int64_t, const_sail_string, bool, bool);

  void startup_zrme_sizze(void);

  void finish_zrme_sizze(void);

  uint64_t zrime_sizze(enum zproc_mode, int64_t, int64_t, uint64_t, struct zoptionzIizK, int64_t, const_sail_string, bool, bool);

  void startup_zrime_sizze(void);

  void finish_zrime_sizze(void);

  unit zwme_sizze(enum zproc_mode, int64_t, int64_t, uint64_t, struct zoptionzIizK, int64_t, lbits, bool, bool);

  void startup_zwme_sizze(void);

  void finish_zwme_sizze(void);

  uint64_t zread_iptr(enum zproc_mode);

  void startup_zread_iptr(void);

  void finish_zread_iptr(void);

  uint64_t zadd_to_iptr(enum zproc_mode, uint64_t, uint64_t);

  void startup_zadd_to_iptr(void);

  void finish_zadd_to_iptr(void);

  unit zwrite_iptr(enum zproc_mode, uint64_t);

  void startup_zwrite_iptr(void);

  void finish_zwrite_iptr(void);

  uint64_t zread_sptr(enum zproc_mode);

  void startup_zread_sptr(void);

  void finish_zread_sptr(void);

  uint64_t zadd_to_sptr(enum zproc_mode, uint64_t, uint64_t);

  void startup_zadd_to_sptr(void);

  void finish_zadd_to_sptr(void);

  unit zwrite_sptr(enum zproc_mode, uint64_t);

  void startup_zwrite_sptr(void);

  void finish_zwrite_sptr(void);

  void zx86_effective_addr_from_sib(struct ztuple_z8z5izCz0z5bv64zCz0z5i64z9 *rop, enum zproc_mode, uint64_t, uint64_t, uint64_t, struct zsib);

  void startup_zx86_effective_addr_from_sib(void);

  void finish_zx86_effective_addr_from_sib(void);

  struct ztuple_z8z5bv64zCz0z5i64z9 zx86_effective_addr_16_disp(enum zproc_mode, uint64_t, uint64_t);

  void startup_zx86_effective_addr_16_disp(void);

  void finish_zx86_effective_addr_16_disp(void);

  struct ztuple_z8z5bv16zCz0z5i64z9 zx86_effective_addr_16(enum zproc_mode, uint64_t, uint64_t, uint64_t);

  void startup_zx86_effective_addr_16(void);

  void finish_zx86_effective_addr_16(void);

  struct ztuple_z8z5bv64zCz0z5i64z9 zx86_effective_addr_32_64(enum zproc_mode, bool, uint64_t, uint64_t, uint64_t, uint64_t, struct zsib, uint64_t);

  void startup_zx86_effective_addr_32_64(void);

  void finish_zx86_effective_addr_32_64(void);

  struct ztuple_z8z5bv64zCz0z5i64z9 zx86_effective_addr(enum zproc_mode, struct zprefixes, uint64_t, uint64_t, uint64_t, uint64_t, struct zsib, uint64_t);

  bool zalignment_checking_enabled_p(unit);

  void startup_zalignment_checking_enabled_p(void);

  void finish_zalignment_checking_enabled_p(void);

  void zx86_operand_from_modr_m_and_sib_bytes(struct ztuple_z8z5bvzCz0z5i64zCz0z5bv64z9 *rop, enum zproc_mode, uint64_t, int64_t, bool, bool, int64_t, struct zprefixes, uint64_t, uint64_t, uint64_t, uint64_t, struct zsib, uint64_t);

  void startup_zx86_operand_from_modr_m_and_sib_bytes(void);

  void finish_zx86_operand_from_modr_m_and_sib_bytes(void);

  unit zx86_operand_to_reg_mem(enum zproc_mode, int64_t, bool, bool, lbits, int64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, uint64_t, struct zsib);

  void startup_zx86_operand_to_reg_mem(void);

  void finish_zx86_operand_to_reg_mem(void);

  unit zx86_operand_to_xmm_mem(enum zproc_mode, int64_t, bool, lbits, int64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, uint64_t, struct zsib);

  void startup_zx86_operand_to_xmm_mem(void);

  void finish_zx86_operand_to_xmm_mem(void);

  int64_t zselect_operand_sizze(enum zproc_mode, bool, uint64_t, bool, struct zprefixes, bool, bool, bool);

  void startup_zselect_operand_sizze(void);

  void finish_zselect_operand_sizze(void);

  void zcheck_instruction_length(struct zoptionzIizK *rop, uint64_t, uint64_t, uint64_t);

  void startup_zcheck_instruction_length(void);

  void finish_zcheck_instruction_length(void);

  unit zx86_add_adc_sub_sbb_or_and_xor_cmp_test_e_g(int64_t, enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_add_adc_sub_sbb_or_and_xor_cmp_test_e_g(void);

  void finish_zx86_add_adc_sub_sbb_or_and_xor_cmp_test_e_g(void);

  unit zx86_add_adc_sub_sbb_or_and_xor_cmp_g_e(int64_t, enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_add_adc_sub_sbb_or_and_xor_cmp_g_e(void);

  void finish_zx86_add_adc_sub_sbb_or_and_xor_cmp_g_e(void);

  unit zx86_add_adc_sub_sbb_or_and_xor_cmp_test_e_i(int64_t, enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_add_adc_sub_sbb_or_and_xor_cmp_test_e_i(void);

  void finish_zx86_add_adc_sub_sbb_or_and_xor_cmp_test_e_i(void);

  unit zx86_add_adc_sub_sbb_or_and_xor_cmp_test_rax_i(int64_t, enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_add_adc_sub_sbb_or_and_xor_cmp_test_rax_i(void);

  void finish_zx86_add_adc_sub_sbb_or_and_xor_cmp_test_rax_i(void);

  unit zx86_inc_dec_fe_ff(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_inc_dec_fe_ff(void);

  void finish_zx86_inc_dec_fe_ff(void);

  unit zx86_inc_dec_4x(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_inc_dec_4x(void);

  void finish_zx86_inc_dec_4x(void);

  unit zx86_not_neg_f6_f7(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_not_neg_f6_f7(void);

  void finish_zx86_not_neg_f6_f7(void);

  unit zx86_bt_0f_a3(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_bt_0f_a3(void);

  void finish_zx86_bt_0f_a3(void);

  unit zx86_bt_0f_ba(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_bt_0f_ba(void);

  void finish_zx86_bt_0f_ba(void);

  bool zjcc_cmovcc_setcc_spec(uint64_t);

  void startup_zjcc_cmovcc_setcc_spec(void);

  void finish_zjcc_cmovcc_setcc_spec(void);

  unit zx86_one_byte_jcc(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_one_byte_jcc(void);

  void finish_zx86_one_byte_jcc(void);

  unit zx86_two_byte_jcc(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_two_byte_jcc(void);

  void finish_zx86_two_byte_jcc(void);

  unit zx86_jrcxzz(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_jrcxzz(void);

  void finish_zx86_jrcxzz(void);

  unit zx86_cmovcc(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_cmovcc(void);

  void finish_zx86_cmovcc(void);

  unit zx86_setcc(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_setcc(void);

  void finish_zx86_setcc(void);

  struct ztuple_z8z5boolzCz0z5bv16zCz0z5bv8z9 zdiv_spec_8(uint64_t, uint64_t);

  void startup_zdiv_spec_8(void);

  void finish_zdiv_spec_8(void);

  struct ztuple_z8z5boolzCz0z5bv32zCz0z5bv16z9 zdiv_spec_16(uint64_t, uint64_t);

  void startup_zdiv_spec_16(void);

  void finish_zdiv_spec_16(void);

  struct ztuple_z8z5boolzCz0z5bv64zCz0z5bv32z9 zdiv_spec_32(uint64_t, uint64_t);

  void startup_zdiv_spec_32(void);

  void finish_zdiv_spec_32(void);

  void zdiv_spec_64(struct ztuple_z8z5boolzCz0z5bvzCz0z5bv64z9 *rop, lbits, uint64_t);

  void startup_zdiv_spec_64(void);

  void finish_zdiv_spec_64(void);

  void zdiv_spec(struct ztuple_z8z5boolzCz0z5bvzCz0z5bv64z9 *rop, int64_t, lbits, uint64_t);

  void startup_zdiv_spec(void);

  void finish_zdiv_spec(void);

  struct ztuple_z8z5boolzCz0z5bv8zCz0z5bv8z9 zidiv_spec_8(uint64_t, uint64_t);

  void startup_zidiv_spec_8(void);

  void finish_zidiv_spec_8(void);

  struct ztuple_z8z5boolzCz0z5bv16zCz0z5bv16z9 zidiv_spec_16(uint64_t, uint64_t);

  void startup_zidiv_spec_16(void);

  void finish_zidiv_spec_16(void);

  struct ztuple_z8z5boolzCz0z5bv32zCz0z5bv32z9 zidiv_spec_32(uint64_t, uint64_t);

  void startup_zidiv_spec_32(void);

  void finish_zidiv_spec_32(void);

  struct ztuple_z8z5boolzCz0z5bv64zCz0z5bv64z9 zidiv_spec_64(lbits, uint64_t);

  void startup_zidiv_spec_64(void);

  void finish_zidiv_spec_64(void);

  struct ztuple_z8z5boolzCz0z5bv64zCz0z5bv64z9 zidiv_spec(int64_t, lbits, uint64_t);

  void startup_zidiv_spec(void);

  void finish_zidiv_spec(void);

  unit zx86_div(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_div(void);

  void finish_zx86_div(void);

  unit zx86_idiv(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_idiv(void);

  void finish_zx86_idiv(void);

  unit zx86_xchg(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_xchg(void);

  void finish_zx86_xchg(void);

  unit zx86_cmpxchg(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_cmpxchg(void);

  void finish_zx86_cmpxchg(void);

  unit zx86_one_byte_nop(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  unit zx86_two_byte_nop(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_two_byte_nop(void);

  void finish_zx86_two_byte_nop(void);

  unit zx86_near_jmp_op_en_d(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_near_jmp_op_en_d(void);

  void finish_zx86_near_jmp_op_en_d(void);

  unit zx86_near_jmp_op_en_m(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_near_jmp_op_en_m(void);

  void finish_zx86_near_jmp_op_en_m(void);

  unit zx86_far_jmp_op_en_d(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_far_jmp_op_en_d(void);

  void finish_zx86_far_jmp_op_en_d(void);

  unit zx86_loop(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_loop(void);

  void finish_zx86_loop(void);

  unit zx86_mov_op_en_mr(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_mov_op_en_mr(void);

  void finish_zx86_mov_op_en_mr(void);

  unit zx86_mov_op_en_rm(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_mov_op_en_rm(void);

  void finish_zx86_mov_op_en_rm(void);

  unit zx86_mov_op_en_fd(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_mov_op_en_fd(void);

  void finish_zx86_mov_op_en_fd(void);

  unit zx86_mov_op_en_td(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_mov_op_en_td(void);

  void finish_zx86_mov_op_en_td(void);

  unit zx86_mov_op_en_oi(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_mov_op_en_oi(void);

  void finish_zx86_mov_op_en_oi(void);

  unit zx86_mov_op_en_mi(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_mov_op_en_mi(void);

  void finish_zx86_mov_op_en_mi(void);

  unit zx86_lea(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_lea(void);

  void finish_zx86_lea(void);

  unit zx86_movsx(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movsx(void);

  void finish_zx86_movsx(void);

  unit zx86_movsxd(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movsxd(void);

  void finish_zx86_movsxd(void);

  unit zx86_movzzx(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movzzx(void);

  void finish_zx86_movzzx(void);

  unit zx86_mov_control_regs_op_en_mr(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_mov_control_regs_op_en_mr(void);

  void finish_zx86_mov_control_regs_op_en_mr(void);

  unit zx86_movd_movq_to_xmm(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movd_movq_to_xmm(void);

  void finish_zx86_movd_movq_to_xmm(void);

  unit zx86_movd_movq_from_xmm(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movd_movq_from_xmm(void);

  void finish_zx86_movd_movq_from_xmm(void);

  struct ztuple_z8z5bv8zCz0z5bv8zCz0z5bv16z9 zmul_spec_8(uint64_t, uint64_t);

  void startup_zmul_spec_8(void);

  void finish_zmul_spec_8(void);

  struct ztuple_z8z5bv16zCz0z5bv16zCz0z5bv32z9 zmul_spec_16(uint64_t, uint64_t);

  void startup_zmul_spec_16(void);

  void finish_zmul_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5bv32zCz0z5bv64z9 zmul_spec_32(uint64_t, uint64_t);

  void startup_zmul_spec_32(void);

  void finish_zmul_spec_32(void);

  void zmul_spec_64(struct ztuple_z8z5bv64zCz0z5bv64zCz0z5bvz9 *rop, uint64_t, uint64_t);

  void startup_zmul_spec_64(void);

  void finish_zmul_spec_64(void);

  void zmul_spec(struct ztuple_z8z5bv64zCz0z5bv64zCz0z5bvz9 *rop, int64_t, uint64_t, uint64_t);

  void startup_zmul_spec(void);

  void finish_zmul_spec(void);

  struct ztuple_z8z5bv8zCz0z5bv8zCz0z5bv16zCz0z5bv1z9 zimul_spec_8(uint64_t, uint64_t);

  void startup_zimul_spec_8(void);

  void finish_zimul_spec_8(void);

  struct ztuple_z8z5bv16zCz0z5bv16zCz0z5bv32zCz0z5bv1z9 zimul_spec_16(uint64_t, uint64_t);

  void startup_zimul_spec_16(void);

  void finish_zimul_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5bv32zCz0z5bv64zCz0z5bv1z9 zimul_spec_32(uint64_t, uint64_t);

  void startup_zimul_spec_32(void);

  void finish_zimul_spec_32(void);

  void zimul_spec_64(struct ztuple_z8z5bv64zCz0z5bv64zCz0z5bvzCz0z5bv1z9 *rop, uint64_t, uint64_t);

  void startup_zimul_spec_64(void);

  void finish_zimul_spec_64(void);

  void zimul_spec(struct ztuple_z8z5bv64zCz0z5bv64zCz0z5bvzCz0z5bv1z9 *rop, int64_t, uint64_t, uint64_t);

  void startup_zimul_spec(void);

  void finish_zimul_spec(void);

  unit zx86_mul(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_mul(void);

  void finish_zx86_mul(void);

  unit zx86_imul_op_en_m(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_imul_op_en_m(void);

  void finish_zx86_imul_op_en_m(void);

  unit zx86_imul_op_en_rm(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_imul_op_en_rm(void);

  void finish_zx86_imul_op_en_rm(void);

  unit zx86_imul_op_en_rmi(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_imul_op_en_rmi(void);

  void finish_zx86_imul_op_en_rmi(void);

  unit zx86_push_general_register(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_push_general_register(void);

  void finish_zx86_push_general_register(void);

  unit zx86_push_ev(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_push_ev(void);

  void finish_zx86_push_ev(void);

  unit zx86_push_i(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_push_i(void);

  void finish_zx86_push_i(void);

  unit zx86_push_segment_register(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_push_segment_register(void);

  void finish_zx86_push_segment_register(void);

  unit zx86_pop_general_register(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_pop_general_register(void);

  void finish_zx86_pop_general_register(void);

  unit zx86_pop_ev(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_pop_ev(void);

  void finish_zx86_pop_ev(void);

  unit zx86_pushf(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_pushf(void);

  void finish_zx86_pushf(void);

  unit zx86_popf(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_popf(void);

  void finish_zx86_popf(void);

  unit zx86_pusha(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_pusha(void);

  void finish_zx86_pusha(void);

  unit zx86_popa(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_popa(void);

  void finish_zx86_popa(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsal_shl_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsal_shl_spec_8(void);

  void finish_zsal_shl_spec_8(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsal_shl_spec_16(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsal_shl_spec_16(void);

  void finish_zsal_shl_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsal_shl_spec_32(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsal_shl_spec_32(void);

  void finish_zsal_shl_spec_32(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsal_shl_spec_64(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsal_shl_spec_64(void);

  void finish_zsal_shl_spec_64(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsal_shl_spec(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsal_shl_spec(void);

  void finish_zsal_shl_spec(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshr_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshr_spec_8(void);

  void finish_zshr_spec_8(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshr_spec_16(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshr_spec_16(void);

  void finish_zshr_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshr_spec_32(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshr_spec_32(void);

  void finish_zshr_spec_32(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshr_spec_64(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshr_spec_64(void);

  void finish_zshr_spec_64(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshr_spec(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshr_spec(void);

  void finish_zshr_spec(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsar_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsar_spec_8(void);

  void finish_zsar_spec_8(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsar_spec_16(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsar_spec_16(void);

  void finish_zsar_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsar_spec_32(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsar_spec_32(void);

  void finish_zsar_spec_32(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsar_spec_64(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsar_spec_64(void);

  void finish_zsar_spec_64(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zsar_spec(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zsar_spec(void);

  void finish_zsar_spec(void);

  struct ztuple_z8z5bv16zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshld_spec_16(uint64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshld_spec_16(void);

  void finish_zshld_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshld_spec_32(uint64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshld_spec_32(void);

  void finish_zshld_spec_32(void);

  struct ztuple_z8z5bv64zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshld_spec_64(uint64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshld_spec_64(void);

  void finish_zshld_spec_64(void);

  struct ztuple_z8z5bv64zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshld_spec(int64_t, uint64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshld_spec(void);

  void finish_zshld_spec(void);

  struct ztuple_z8z5bv16zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshrd_spec_16(uint64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshrd_spec_16(void);

  void finish_zshrd_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshrd_spec_32(uint64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshrd_spec_32(void);

  void finish_zshrd_spec_32(void);

  struct ztuple_z8z5bv64zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshrd_spec_64(uint64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshrd_spec_64(void);

  void finish_zshrd_spec_64(void);

  struct ztuple_z8z5bv64zCz0z5boolzCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zshrd_spec(int64_t, uint64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zshrd_spec(void);

  void finish_zshrd_spec(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcl_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcl_spec_8(void);

  void finish_zrcl_spec_8(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcl_spec_16(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcl_spec_16(void);

  void finish_zrcl_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcl_spec_32(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcl_spec_32(void);

  void finish_zrcl_spec_32(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcl_spec_64(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcl_spec_64(void);

  void finish_zrcl_spec_64(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcl_spec(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcl_spec(void);

  void finish_zrcl_spec(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrol_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrol_spec_8(void);

  void finish_zrol_spec_8(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrol_spec_16(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrol_spec_16(void);

  void finish_zrol_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrol_spec_32(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrol_spec_32(void);

  void finish_zrol_spec_32(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrol_spec_64(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrol_spec_64(void);

  void finish_zrol_spec_64(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrol_spec(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrol_spec(void);

  void finish_zrol_spec(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcr_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcr_spec_8(void);

  void finish_zrcr_spec_8(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcr_spec_16(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcr_spec_16(void);

  void finish_zrcr_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcr_spec_32(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcr_spec_32(void);

  void finish_zrcr_spec_32(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcr_spec_64(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcr_spec_64(void);

  void finish_zrcr_spec_64(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zrcr_spec(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zrcr_spec(void);

  void finish_zrcr_spec(void);

  struct ztuple_z8z5bv8zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zror_spec_8(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zror_spec_8(void);

  void finish_zror_spec_8(void);

  struct ztuple_z8z5bv16zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zror_spec_16(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zror_spec_16(void);

  void finish_zror_spec_16(void);

  struct ztuple_z8z5bv32zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zror_spec_32(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zror_spec_32(void);

  void finish_zror_spec_32(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zror_spec_64(uint64_t, uint64_t, struct zrflagsbits);

  void startup_zror_spec_64(void);

  void finish_zror_spec_64(void);

  struct ztuple_z8z5bv64zCz0z5structz0zzrflagsbitszCz0z5structz0zzrflagsbitsz9 zror_spec(int64_t, uint64_t, uint64_t, struct zrflagsbits);

  void startup_zror_spec(void);

  void finish_zror_spec(void);

  unit zx86_sal_sar_shl_shr_rcl_rcr_rol_ror(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_sal_sar_shl_shr_rcl_rcr_rol_ror(void);

  void finish_zx86_sal_sar_shl_shr_rcl_rcr_rol_ror(void);

  unit zx86_shld_shrd(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_shld_shrd(void);

  void finish_zx86_shld_shrd(void);

  unit zx86_lgdt(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_lgdt(void);

  void finish_zx86_lgdt(void);

  unit zx86_lidt(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_lidt(void);

  void finish_zx86_lidt(void);

  unit zx86_lldt(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_lldt(void);

  void finish_zx86_lldt(void);

  unit zx86_cbw_cwd_cdqe(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_cbw_cwd_cdqe(void);

  void finish_zx86_cbw_cwd_cdqe(void);

  unit zx86_cwd_cdq_cqo(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_cwd_cdq_cqo(void);

  void finish_zx86_cwd_cdq_cqo(void);

  unit zx86_movs(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movs(void);

  void finish_zx86_movs(void);

  unit zx86_cmps(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_cmps(void);

  void finish_zx86_cmps(void);

  unit zx86_stos(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_stos(void);

  void finish_zx86_stos(void);

  unit zx86_syscall(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_syscall(void);

  void finish_zx86_syscall(void);

  unit zx86_syscall_both_views(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  unit zx86_sysret(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_sysret(void);

  void finish_zx86_sysret(void);

  unit zx86_call_e8_op_en_m(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_call_e8_op_en_m(void);

  void finish_zx86_call_e8_op_en_m(void);

  unit zx86_call_ff_2_op_en_m(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_call_ff_2_op_en_m(void);

  void finish_zx86_call_ff_2_op_en_m(void);

  unit zx86_ret(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_ret(void);

  void finish_zx86_ret(void);

  unit zx86_leave(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_leave(void);

  void finish_zx86_leave(void);

  uint64_t zextract_32_bits(lbits, uint64_t);

  void startup_zextract_32_bits(void);

  void finish_zextract_32_bits(void);

  uint64_t zextract_64_bits(lbits, int64_t);

  void startup_zextract_64_bits(void);

  void finish_zextract_64_bits(void);

  unit zx86_shufps_op_en_rmi(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_shufps_op_en_rmi(void);

  void finish_zx86_shufps_op_en_rmi(void);

  unit zx86_shufpd_op_en_rmi(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_shufpd_op_en_rmi(void);

  void finish_zx86_shufpd_op_en_rmi(void);

  unit zx86_unpckzLps_op_en_rm(int64_t, enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_unpckzLps_op_en_rm(void);

  void finish_zx86_unpckzLps_op_en_rm(void);

  unit zx86_unpckzLpd_op_en_rm(int64_t, enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_unpckzLpd_op_en_rm(void);

  void finish_zx86_unpckzLpd_op_en_rm(void);

  unit zx86_movss_movsd_op_en_rm(int64_t, enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movss_movsd_op_en_rm(void);

  void finish_zx86_movss_movsd_op_en_rm(void);

  unit zx86_movss_movsd_op_en_mr(int64_t, enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movss_movsd_op_en_mr(void);

  void finish_zx86_movss_movsd_op_en_mr(void);

  unit zx86_movaps_movapd_op_en_rm(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movaps_movapd_op_en_rm(void);

  void finish_zx86_movaps_movapd_op_en_rm(void);

  unit zx86_movaps_movapd_op_en_mr(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movaps_movapd_op_en_mr(void);

  void finish_zx86_movaps_movapd_op_en_mr(void);

  unit zx86_movups_movupd_movdqu_op_en_rm(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movups_movupd_movdqu_op_en_rm(void);

  void finish_zx86_movups_movupd_movdqu_op_en_rm(void);

  unit zx86_movups_movupd_movdqu_op_en_mr(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movups_movupd_movdqu_op_en_mr(void);

  void finish_zx86_movups_movupd_movdqu_op_en_mr(void);

  unit zx86_movlps_movlpd_op_en_rm(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movlps_movlpd_op_en_rm(void);

  void finish_zx86_movlps_movlpd_op_en_rm(void);

  unit zx86_movlps_movlpd_op_en_mr(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movlps_movlpd_op_en_mr(void);

  void finish_zx86_movlps_movlpd_op_en_mr(void);

  unit zx86_movhps_movhpd_op_en_rm(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movhps_movhpd_op_en_rm(void);

  void finish_zx86_movhps_movhpd_op_en_rm(void);

  unit zx86_movhps_movhpd_op_en_mr(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_movhps_movhpd_op_en_mr(void);

  void finish_zx86_movhps_movhpd_op_en_mr(void);

  unit zx86_hlt(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  unit zx86_cmc_clc_stc_cld_std(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_cmc_clc_stc_cld_std(void);

  void finish_zx86_cmc_clc_stc_cld_std(void);

  unit zx86_sahf(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_sahf(void);

  void finish_zx86_sahf(void);

  unit zx86_lahf(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zx86_lahf(void);

  void finish_zx86_lahf(void);

  uint64_t zn64_bit_compute_mandatory_prefix_for_two_byte_opcode(uint64_t, struct zprefixes);

  void startup_zn64_bit_compute_mandatory_prefix_for_two_byte_opcode(void);

  void finish_zn64_bit_compute_mandatory_prefix_for_two_byte_opcode(void);

  uint64_t zn32_bit_compute_mandatory_prefix_for_two_byte_opcode(uint64_t, struct zprefixes);

  void startup_zn32_bit_compute_mandatory_prefix_for_two_byte_opcode(void);

  void finish_zn32_bit_compute_mandatory_prefix_for_two_byte_opcode(void);

  unit zchk_exc_fn(const_sail_string, const_sail_string, zz5listz8z5stringz9, enum zproc_mode, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zchk_exc_fn(void);

  void finish_zchk_exc_fn(void);

  int64_t zget_one_byte_prefix_array_code(uint64_t);

  void startup_zget_one_byte_prefix_array_code(void);

  void finish_zget_one_byte_prefix_array_code(void);

  uint64_t zcompute_mandatory_prefix_for_two_byte_opcode(enum zproc_mode, uint64_t, struct zprefixes);

  bool zn64_bit_mode_one_byte_opcode_modr_m_p(uint64_t);

  void startup_zn64_bit_mode_one_byte_opcode_modr_m_p(void);

  void finish_zn64_bit_mode_one_byte_opcode_modr_m_p(void);

  bool zn32_bit_mode_one_byte_opcode_modr_m_p(uint64_t);

  void startup_zn32_bit_mode_one_byte_opcode_modr_m_p(void);

  void finish_zn32_bit_mode_one_byte_opcode_modr_m_p(void);

  bool zone_byte_opcode_modr_m_p(enum zproc_mode, uint64_t);

  bool zn64_bit_mode_two_byte_opcode_modr_m_p(uint64_t, uint64_t);

  void startup_zn64_bit_mode_two_byte_opcode_modr_m_p(void);

  void finish_zn64_bit_mode_two_byte_opcode_modr_m_p(void);

  bool zn32_bit_mode_two_byte_opcode_modr_m_p(uint64_t, uint64_t);

  void startup_zn32_bit_mode_two_byte_opcode_modr_m_p(void);

  void finish_zn32_bit_mode_two_byte_opcode_modr_m_p(void);

  bool ztwo_byte_opcode_modr_m_p(enum zproc_mode, uint64_t, uint64_t);

  bool zvex_opcode_modr_m_p(struct zvex_prefixes, uint64_t);

  bool zx86_decode_sib_p(struct zmodr_m, bool);

  unit ztwo_byte_opcode_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_ztwo_byte_opcode_execute(void);

  void finish_ztwo_byte_opcode_execute(void);

  unit ztwo_byte_opcode_decode_and_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t);

  void startup_ztwo_byte_opcode_decode_and_execute(void);

  void finish_ztwo_byte_opcode_decode_and_execute(void);

  unit zvex_0f_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zvex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  void startup_zvex_0f_execute(void);

  void finish_zvex_0f_execute(void);

  unit zvex_0f38_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zvex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  void startup_zvex_0f38_execute(void);

  void finish_zvex_0f38_execute(void);

  unit zvex_0f3a_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zvex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  void startup_zvex_0f3a_execute(void);

  void finish_zvex_0f3a_execute(void);

  unit zvex_decode_and_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zvex_prefixes);

  void startup_zvex_decode_and_execute(void);

  void finish_zvex_decode_and_execute(void);

  unit zevex_0f_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zevex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  void startup_zevex_0f_execute(void);

  void finish_zevex_0f_execute(void);

  unit zevex_0f38_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zevex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  void startup_zevex_0f38_execute(void);

  void finish_zevex_0f38_execute(void);

  unit zevex_0f3a_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zevex_prefixes, uint64_t, struct zmodr_m, struct zsib);

  void startup_zevex_0f3a_execute(void);

  void finish_zevex_0f3a_execute(void);

  unit zevex_decode_and_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, struct zevex_prefixes);

  void startup_zevex_decode_and_execute(void);

  void finish_zevex_decode_and_execute(void);

  struct ztuple_z8z5structz0zzprefixeszCz0z5bv8z9 zget_prefixes(enum zproc_mode, uint64_t, struct zprefixes, uint64_t, int64_t);

  unit zone_byte_opcode_execute(enum zproc_mode, uint64_t, uint64_t, struct zprefixes, uint64_t, uint64_t, struct zmodr_m, struct zsib);

  void startup_zone_byte_opcode_execute(void);

  void finish_zone_byte_opcode_execute(void);

  unit zx86_fetch_decode_execute(unit);

  void startup_zx86_fetch_decode_execute(void);

  void finish_zx86_fetch_decode_execute(void);

  unit zinitialise_64_bit_mode(unit);

  void startup_zinitialise_64_bit_mode(void);

  void finish_zinitialise_64_bit_mode(void);

  void startup_z__SetConfig(void);

  void finish_z__SetConfig(void);

  bool zx86_step_print_halt(uint64_t, bool, bool, sail_int);

  void startup_zx86_step_print_halt(void);

  void finish_zx86_step_print_halt(void);

  void zx86_run_halt(sail_int *rop, uint64_t, sail_int, bool, bool, sail_int);

  unit zmain(unit);

  void startup_zmain(void);

  void finish_zmain(void);

  unit zinit_harness(unit);

  void startup_zinit_harness(void);

  void finish_zinit_harness(void);

  unit zset_gpr(uint64_t, uint64_t);

  void startup_zset_gpr(void);

  void finish_zset_gpr(void);

  uint64_t zget_gpr(uint64_t);

  void startup_zget_gpr(void);

  void finish_zget_gpr(void);

  unit zset_rip(uint64_t);

  void startup_zset_rip(void);

  void finish_zset_rip(void);

  uint64_t zget_rip(unit);

  unit zset_rflags(uint64_t);

  uint64_t zget_rflags(unit);

  unit zset_zzmm_chunk(uint64_t, uint64_t, uint64_t);

  void startup_zset_zzmm_chunk(void);

  void finish_zset_zzmm_chunk(void);

  uint64_t zget_zzmm_chunk(uint64_t, uint64_t);

  void startup_zget_zzmm_chunk(void);

  void finish_zget_zzmm_chunk(void);

  unit zset_msr(uint64_t, uint64_t);

  void startup_zset_msr(void);

  void finish_zset_msr(void);

  uint64_t zget_msr(uint64_t);

  void startup_zget_msr(void);

  void finish_zget_msr(void);

  unit zset_ctr(uint64_t, uint64_t);

  void startup_zset_ctr(void);

  void finish_zset_ctr(void);

  uint64_t zget_ctr(uint64_t);

  void startup_zget_ctr(void);

  void finish_zget_ctr(void);

  unit zset_seg_visible(uint64_t, uint64_t);

  void startup_zset_seg_visible(void);

  void finish_zset_seg_visible(void);

  uint64_t zget_seg_visible(uint64_t);

  void startup_zget_seg_visible(void);

  void finish_zget_seg_visible(void);

  unit zset_seg_base(uint64_t, uint64_t);

  void startup_zset_seg_base(void);

  void finish_zset_seg_base(void);

  uint64_t zget_seg_base(uint64_t);

  void startup_zget_seg_base(void);

  void finish_zget_seg_base(void);

  unit zset_seg_limit(uint64_t, uint64_t);

  void startup_zset_seg_limit(void);

  void finish_zset_seg_limit(void);

  uint64_t zget_seg_limit(uint64_t);

  void startup_zget_seg_limit(void);

  void finish_zget_seg_limit(void);

  unit zset_seg_attr(uint64_t, uint64_t);

  void startup_zset_seg_attr(void);

  void finish_zset_seg_attr(void);

  uint64_t zget_seg_attr(uint64_t);

  void startup_zget_seg_attr(void);

  void finish_zget_seg_attr(void);

  void zget_fault_msg(sail_string *rop, unit);

  uint64_t zstep_x86(unit);

  unit zinitializze_registers(unit);

  void startup_zinitializze_registers(void);

  void finish_zinitializze_registers(void);

  Model();

  ~Model();

  Model(const Model&) = delete;

  struct zexception *current_exception = NULL;

  bool have_exception = false;

  sail_string *throw_location = NULL;

  // register zapp_view
  bool zapp_view = {};

  // register zmarking_view
  bool zmarking_view = {};

  // register zlog_register_writes
  bool zlog_register_writes = {};

  // register zms_reg
  bool zms_reg = {};

  // register zfault_reg
  bool zfault_reg = {};

  // register zrflags
  struct zrflagsbits zrflags = {};

  // register zrip
  uint64_t zrip = {};

  // register zrax
  uint64_t zrax = {};

  // register zrbx
  uint64_t zrbx = {};

  // register zrcx
  uint64_t zrcx = {};

  // register zrdx
  uint64_t zrdx = {};

  // register zrsi
  uint64_t zrsi = {};

  // register zrdi
  uint64_t zrdi = {};

  // register zrsp
  uint64_t zrsp = {};

  // register zrbp
  uint64_t zrbp = {};

  // register zr8
  uint64_t zr8 = {};

  // register zr9
  uint64_t zr9 = {};

  // register zr10
  uint64_t zr10 = {};

  // register zr11
  uint64_t zr11 = {};

  // register zr12
  uint64_t zr12 = {};

  // register zr13
  uint64_t zr13 = {};

  // register zr14
  uint64_t zr14 = {};

  // register zr15
  uint64_t zr15 = {};

  // register zmsrs
  zz5vecz8z5bv64z9 zmsrs = {};

  // register zseg_visibles
  zz5vecz8z5bv16z9 zseg_visibles = {};

  // register zseg_hidden_attrs
  zz5vecz8z5bv16z9 zseg_hidden_attrs = {};

  // register zseg_hidden_bases
  zz5vecz8z5bv64z9 zseg_hidden_bases = {};

  // register zseg_hidden_limits
  zz5vecz8z5bv32z9 zseg_hidden_limits = {};

  // register zzzmms
  zz5vecz8z5bvz9 zzzmms = {};

  // register zctrs
  zz5vecz8z5bv64z9 zctrs = {};

  // register zstrs
  zz5vecz8z5bvz9 zstrs = {};

  // register zssr_visibles
  zz5vecz8z5bv16z9 zssr_visibles = {};

  // register zssr_hidden_bases
  zz5vecz8z5bv64z9 zssr_hidden_bases = {};

  // register zssr_hidden_limits
  zz5vecz8z5bv32z9 zssr_hidden_limits = {};

  // register zssr_hidden_attrs
  zz5vecz8z5bv16z9 zssr_hidden_attrs = {};

  // register zhaltAddrReg
  uint64_t zhaltAddrReg = {};

  // register zlast_exception
  sail_string zlast_exception = {};

  inline static const size_t SAIL_TEST_COUNT = 0;
  inline static unit (Model::*const SAIL_TESTS[1])(unit) = {
    NULL
  };
  inline static const char* const SAIL_TEST_NAMES[1] = {
    NULL
  };

  void model_init();
  void model_fini();
  };
} // namespace


