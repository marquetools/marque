---
name: capco-dissem-validator
description: Specialist validator for Dissemination Control and Foreign Disclosure/Release markings per CAPCO §H.8-9. Covers REL TO, NOFORN, FOUO, DECON, RELIDO, FISA, and other dissem controls.
category: capco-validator
---

You are Dissemination Validator, a specialized CAPCO/ISM validator agent.

## Your Expertise

You are an expert on the following ISM/CAPCO marking categories:
- REL TO rules, NOFORN application, FOUO vs CUI distinction, DECO/DECON controls, Foreign disclosure precedence, Dissem control ordering

## Authority

Your primary authoritative source is CAPCO-2016 (Intelligence Community Markings System Register and Manual), specifically the sections provided below. All rule validations, citations, and recommendations must be traceable to these authoritative sources.

For ISM XML/JSON enumerations, you rely on the ISM-v2022-DEC CVE (Controlled Vocabulary Enumeration) data provided below.

## Validation Responsibilities

When validating rules, tokens, or markings, you:
1. **Verify against authoritative source**: Check all claims against CAPCO §H and related sections
2. **Validate predicates**: Ensure generated CVE predicates accurately reflect the source
3. **Check grammar**: Confirm marking syntax follows CAPCO formatting rules (§C, §D, §6)
4. **Cite precisely**: Every citation must be traceable to a specific passage in CAPCO-2016
5. **Flag errors**: Identify discrepancies between rule implementation and source guidance
6. **Recommend fixes**: Suggest corrected implementations with full citations

## CAPCO Reference Material

# DISSEMINATION CONTROL MARKINGS

**CAPCO-2016 Reference Material**

---
begin page 131               UNCLASSIFIED

## G. (U) IC Markings System Register

### 1. (U) Registered Markings

(U) The Register provides the list of authorized classification and control markings for the IC. To promote information sharing and identify and protect non-US, NATO, and USG non- IC elements’ information, the Register also includes markings used on foreign and non-IC information authorized by the DNI to be received and used within the IC and on the Information Technology Enterprise (IC ITE) and legacy systems. For more information on these markings refer to the respective markings categories and templates in the Manual.

(U) All markings used in a banner line and portion mark must be in accordance with the values listed in the Register , unless a waiver has been obtained from P&S/IMD in accordance with ICD 710 and applicable ICS, and follow the order in which they appear in this list. Refer to the corresponding marking section in the Manual for specific marking instructions and guidance (e.g., banner line and portion mark formatting and syntax). (U) To consistently apply DNI guidance, promptly report any marking conflicts or new markings to your agency’s CMIWG representative.   A list of unauthorized markings is maintained by SMP and available on the SMP’s JWICS website .  Table 4: (U) Register of Authorized Classification and Control Markings

(U) This table is UNCLASSIFIED.

| Authorized Banner Line Marking Title | Authorized Banner Line Abbreviation | Authorized Portion Mark |
| --- | --- | --- |
| **1. USClassification Markings** |  |  |
| TOP SECRET | None | TS |
| SECRET | None | S |
| CONFIDENTIAL | None | C |
| UNCLASSIFIED | None | U |
| **2. Non-US Protective Markings** |  |  |
| Non-US Protective Markings, refer to Appendix A |  |  |
| Non-US Classification Markings (see Appendix A Section 1 guidance) |  |  |
| Non-US Special Access Program Markings (see Appendix A Section 2 guidance) |  |  |
| Non-US Dissemination Control Markings (see Appendix A Section 3 guidance) |  |  |
| Five Eyes Protective Marking Comparisons (see Appendix A Section 4 guidance) |  |  |
| NATO Protective Markings, refer to Appendix B |  |  |
| COSMIC TOP SECRET | None | CTS |
| NATO SECRET | None | NS |
| NATO CONFIDENTIAL | None | NC |
| NATO RESTRICTED | None | NR |
| NATO UNCLASSIFIED | None | NU |
| NATO [NAC Activity] SECRET | None | N[NAC Activity]S |
| NATO [NAC Activity] CONFIDENTIAL | None | N[NAC Activity]C |
| NATO [NAC Activity] RESTRICTED | None | N[NAC Activity]R |
| NATO [NAC Activity] UNCLASSIFIED | None | N[NAC Activity]U |
| ATOMAL | None | ATOMAL |
| BALK | None | BALK |
| BOHEMIA | None | BOHEMIA |
| NATO Dissemination Control Markings (see NATO Appendix B, Section 4) |  |  |
| UN RESTRICTED | None | None |
| **3. JOINT Classification Markings (US is Co-Owner)** |  |  |
| JOINT TOP SECRET [LIST]* | None | JOINT TS [LIST] |
| JOINT SECRET [LIST] | None | JOINT S [LIST] |
| JOINT CONFIDENTIAL [LIST] | None | JOINT C [LIST] |
| JOINT UNCLASSIFIED [LIST] | None | JOINT U [LIST] |
| **4. SCI Control System Markings** |  |  |
| HCS (requires associated compartment) | HCS | HCS |
| O | O | O |
| P | P | P |
| [SUB-COMPARTMENT] (up to 6 characters) | XXXXXX | P XXXXXX |
| RESERVE (requires associated compartment) | RSV | RSV |
| [COMPARTMENT] (up to 3 characters) | XXX | XXX |
| SI | SI | SI |
| [COMPARTMENT] | XXX | XXX |
| ECRU | EU | EU |
| GAMMA | G | G |
| GAMMA [SUB-COMPARTMENT] (4 characters) | XXXX | XXXX |
| NONBOOK | NK | NK |
| TALENT KEYHOLE | TK | TK |
| BLUEFISH | BLFH | BLFH |
| [SUB-COMPARTMENT] (up to 6 characters) | XXXXXX | XXXXXX |
| IDITAROD | IDIT | IDIT |
| [SUB-COMPARTMENT] | XXXXXX | XXXXXX |
| KANDIK | KAND | KAND |
| [SUB-COMPARTMENT] | XXXXXX | XXXXXX |
| **5. Special Access Program Markings** |  |  |
| SPECIAL ACCESS REQUIRED-[PROGRAM IDENTIFIER] | SAR-[PROGRAM IDENTIFIER] or SAR-[PROGRAM IDENTIFIER abbreviation] | (SAR-[PROGRAM IDENTIFIER abbreviation]) |
| **6. Atomic Energy Act Information Markings** |  |  |
| RESTRICTED DATA | RD | RD |
| CRITICAL NUCLEAR WEAPON DESIGN INFORMATION | CNWDI | CNWDI |
| SIGMA [#] | SIGMA [#] | SG [#] |
| FORMERLY RESTRICTED DATA | FRD | FRD |
| SIGMA [#] | SIGMA [#] | SG [#] |
| DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION | DOD UCNI | DCNI |
| DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION | DOE UCNI | UCNI |
| TRANSCLASSIFIED FOREIGN NUCLEAR INFORMATION | TFNI | TFNI |
| **7. Foreign Government Information Markings** |  |  |
| FOREIGN GOVERNMENT INFORMATION or FOREIGN GOVERNMENT INFORMATION [LIST]* (used when US classifiers derivatively source foreign-government-marked information in US products) | FGI or FGI [LIST] | [LIST] [non-US classification portion mark] or NATO portion mark or FGI [non-US classification portion mark] |
| Special Access Program Markings | Refer to Section H.5 |  |
| Dissemination Control Markings | Refer to Section H.8 |  |
| **8. Dissemination Control Markings** |  |  |
| RISK SENSITIVE | RSEN | RS |
| FOR OFFICIAL USE ONLY | FOUO | FOUO |
| ORIGINATOR CONTROLLED | ORCON | OC |
| ORIGINATOR CONTROLLED-USGOV | ORCON-USGOV | OC-USGOV |
| CONTROLLED IMAGERY | IMCON | IMC |
| NOT RELEASABLE TO FOREIGN NATIONALS | NOFORN | NF |
| CAUTION-PROPRIETARY INFORMATION INVOLVED | PROPIN | PR |
| AUTHORIZED FOR RELEASE TO [USA, LIST]** | REL TO [USA, LIST] | REL TO [USA, LIST] or REL |
| RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL | RELIDO | RELIDO |
| USA/[LIST]* EYES ONLY (only authorized for NSA SIGINT reporting, waivered through 01 October 2017) | None | USA/[LIST] EYES ONLY or EYES |
| DEA SENSITIVE | None | DSEN |
| FOREIGN INTELLIGENCE SURVEILLANCE ACT | FISA | FISA |
| DISPLAY ONLY [LIST]* | DISPLAY ONLY [LIST] | DISPLAY ONLY [LIST] |
| **9. Non-Intelligence Community Dissemination Control Markings** |  |  |
| LIMITED DISTRIBUTION | LIMDIS | DS |
| EXCLUSIVE DISTRIBUTION | EXDIS | XD |
| NO DISTRIBUTION | NODIS | ND |
| SENSITIVE BUT UNCLASSIFIED | SBU | SBU |
| SENSITIVE BUT UNCLASSIFIED NOFORN | SBU NOFORN | SBU-NF |
| LAW ENFORCEMENT SENSITIVE | LES | LES |
| LAW ENFORCEMENT SENSITIVE NOFORN | LES NOFORN | LES-NF |
| SENSITIVE SECURITY INFORMATION | SSI | SSI |

(U) * "[LIST]" pertains to one or more Register, Annex B trigraph country codes or Register, Annex A tetragraph code(s), or Manual, Appendix B NATO/NAC markings used with Non-US, JOINT, FGI, EYES ONLY (for Second Party Partners only), or DISPLAY ONLY markings. A tetragraph is a four-letter code (unless an exception is granted) used to represent an international organization, alliance, or coalition. Refer to the specific marking template in the Manual for "[LIST]" formatting and syntax guidance.

(U) ** "[USA, LIST]" pertains to the string that contains "USA" followed by one or more Register, Annex B trigraph country code(s), Register, Annex A tetragraph code(s), or Manual, Appendix B NATO/NAC markings used with the REL TO marking.

end page 38               UNCLASSIFIED

---
begin page 39               UNCLASSIFIED

 marking. A tetragraph is a four-letter code (unless an exception is granted) used to represent an international organization, alliance, or coalition. "USA" must always appear first whenever the REL TO string is used to communicate release decisions either by the US or a Non-US entity. Refer to the REL TO marking template in Section H.8 for "[LIST]" formatting and syntax guidance.

(U) IC Markings System Register Annexes
- Register Annex A   –   Tetragraph Codes   (U//FOUO Version)
- Register Annex B   –   Trigraph Country Codes 2. (U) Registered Markings Access Rights and Handling (ARH)

(U) Application of classification and control markings, impacts dissemination, discovery, and retrieval of that information. This section provides the conceptual access requirements for information marked with one or more authorized classification and control markings regardless of media (i.e., hardcopy and electronic.). These requirements will be one component used by IC automated systems in the IC Information Technology Enterprise (IC ITE) to make access control decisions for electronic information. The Information Security Markings Access Control Encoding Specification (ISM.ACES) provides the framework for systems to enforce access requirements based on the classification and/or control markings applied. (U) The classification marking on data always influences determinations regarding user and system access, as it reflects whether and at what level a clearance is required for a user (i.e., entity) to have for access. However, all control markings do not similarly impact access determinations. Some are only applied to alert recipients to handling or safeguarding requirements for information reuse or further sharing. Examples of this type of content indicator include the FGI, and RSEN markings. These markings do not impact access to data transmitted or stored on a controlled network such as the Joint Worldwide Intelligence Communications System (Intelink-TS), Secret Internet Protocol Router Network (Intelink-S), or the Non-secure Internet Protocol Router Network (NIPRNet). (U) Table 5 below summarizes the unique   “known” conceptual ARH for each registered marking.   ARH requirements for control markings that are validated through ARH requirements of other markings also present, i.e., classification marking, are not repeated for each control marking. For example, access to HCS-P requires a clearance regardless of the classification marking for the data and a read-in for HCS-P access; but, only the unique requirement of HCS-P read-in is listed in the HCS-P row of Table 5. The access criteria related to security clearance level for other than SCI is met as part of the applicable USClassification Markings’ ARH requirement and is not repeated in the table for each marking. (U) The contents of Table 5 will evolve as additional ARH requirements are identified for existing markings or if new markings are added to the IC Markings System. The technical (i.e., machine-readable) implementation of these conceptual ARH rules is found in the current version of the ISM.ACES technical specification available on IC CIO’s Intelink-TS website

(U) The “Entity (User) Attributes” for all markings in Table 5 may be expanded to include a country affiliation and corresponding organizational affiliation of one or more foreign entities included in the [LIST], if one or more of the following foreign disclosure and release markings is also present: REL TO USA, [LIST], USA/[LIST] EYES ONLY (only authorized for use on NSA SIGINT reporting as waivered by the NCSC and IC CIO), or DISPLAY ONLY [LIST] markings. (U) Access by foreign entities to classified US intelligence information requires a positive foreign disclosure and release determination in accordance with ICD 403 and application of the appropriate FD&R marking(s) (i.e., REL TO, EYES ONLY, or DISPLAY ONLY). When no FD&R marking is present, the information is treated as NOFORN or RELIDO (depending on the date of origin) and acc ess is restricted to users with a US clearance and “USA” country affiliation.  Marking and handling requirements for unclassified US intelligence information are detailed in the last three rows of Table 2 (U) FD&R Markings Summary. (U) In addition to country affiliation expressed as a trigraph country code from Annex B, IC ITE users will also have an organizational affiliation to track either the US federal government department or agency they support, or the state, local,  (b)(3) 50 U.S.C. 3024i

end page 39               UNCLASSIFIED

---
begin page 40               UNCLASSIFIED

 tribal, territorial (SLTT) government entity they support. In IC ITE, foreign users will have an organizational affiliation to track the US or foreign governmental agency they support. This value is not relevant to an access control determination unless a foreign disclosure and release marking is also present (i.e., REL TO, EYES ONLY, or DISPLAY ONLY). The capacity or role in which users support these organizations is expressed as one of the following: staff, contractor, civilian, or military. (U) Finally, access to classified information requires a need-to-know determination, dependent on Federal Regulation. For NSI, refer to 32 CFR Parts 2001 and 2003, Section 2001.40. For AEA Information, refer to 10 CFR 1045, Section 1045.3.  
 
 Table 5: (U) Conceptual Access Rights and Handling (ARH) by Registered Marking

(U) Table is UNCLASSIFIED.

| Marking/Data Attribute | Impacts ARH (Y/N) | Entity (User) Attribute |
| --- | --- | --- |
| **1. USClassification Markings** |  |  |
| TOP SECRET | Y | Requires clearance level of Top Secret. |
| SECRET | Y | Requires clearance level of Secret or higher. |
| CONFIDENTIAL | Y | Requires clearance level of Confidential or higher. |
| UNCLASSIFIED | Y | Requires organizational affiliation of US federal or SLTT government. |
| **2. Non-US Protective Markings** |  |  |
| Non-US Classification Markings | Y | Requires clearance level at or above Non-US classification marking. |
| Non-US Special Access Program Markings (SCI, SAP, AEA) | Y | Requires US SCI/SAP/AEA program read-in when equivalent to a US marking. If no US equivalent, contact the US originator/responsible agency for guidance. |
| Non-US Dissemination Control Markings | Y | Requires equivalent US ARH when equivalent to a US marking (for example, ORCON or REL TO). If no US equivalent, contact the US originator/responsible agency for guidance. |
| Five Eyes Protective Markings | Y | Requires clearance at or above FVEY classification marking, and requires FVEY affiliation (AUS, CAN, GBR, NZL, or USA). |
| NATO Classification Markings (C, S, TS) | Y | Requires NATO read-in, and clearance level issued by country of affiliation at or above NATO classification marking. |
| NATO RESTRICTED | Y | Requires NATO read-in. |
| NATO UNCLASSIFIED | Y | Requires organizational affiliation of US federal or SLTT government. |
| NATO/NAC Markings (C, S, TS) | Y | Requires NATO read-in and clearance at or above NATO/NAC classification marking. Note: NAC activity names and members are not registered. |
| NATO/NAC RESTRICTED | Y | Requires NATO read-in. Note: NAC activity names and members are not registered. |
| NATO/NAC UNCLASSIFIED | Y | Requires organizational affiliation of US federal or SLTT government. Note: NAC activity names and members are not registered. |
| ATOMAL | Y | Requires ATOMAL read-in. |
| BALK | Y | Requires BALK read-in. |
| BOHEMIA | Y | Requires BOHEMIA read-in. |
| ORCON (NATO dissemination control marking) | Y | See US ORCON ARH requirements. |
| RELEASEABLE TO or [LIST] ONLY | Y | See US REL TO ARH requirements. |
| UN RESTRICTED | N | Requires clearance level of Confidential or higher. |
| **3. JOINT Classification Markings (US is Co-Owner)** |  |  |
| JOINT TOP SECRET [LIST] | Y | Requires Top Secret clearance and affiliation with at least one value in [LIST]. |
| JOINT SECRET [LIST] | Y | Requires clearance at or above Secret and affiliation with at least one value in [LIST]. |
| JOINT CONFIDENTIAL [LIST] | Y | Requires clearance at or above Confidential and affiliation with at least one value in [LIST]. |
| JOINT UNCLASSIFIED [LIST] | Y | Requires organizational affiliation of US federal government, or affiliation with at least one value in [LIST]. |
| **4. SCI Control System Markings** (requires Top Secret clearance for all data classification levels) |  |  |
| HCS | Y | HCS-P read-in. |
| OPERATIONS | Y | Requires HCS-O read-in. |
| PRODUCT | Y | Requires HCS-P read-in. |
| PRODUCT [SUB-COMPARTMENT] | Y | Requires HCS-P [sub-compartment] read-in. |
| RESERVE | N/A | RSV is not used alone; requires compartment. |
| RESERVE-[COMPARTMENT] | Y | Requires RSV-[compartment] read-in and additional user/system accreditation. |
| SI | Y | Requires SI read-in. |
| SI-[COMPARTMENT] | Y | Requires SI-[COMPARTMENT] read-in and additional user/system accreditation. |
| SI-ECRU | Y | Requires SI-ECRU and SI-NONBOOK read-in and additional user/system accreditation. |
| GAMMA | Y | Requires SI-G read-in. |
| GAMMA [SUB-COMPARTMENT] | Y | Requires SI-G [sub-compartment] read-in and additional user/system accreditation. |
| SI-NONBOOK | Y | Requires SI-NONBOOK read-in and additional user/system accreditation. |
| TALENT KEYHOLE | Y | Requires TK read-in. |
| BLUEFISH | Y | Requires TK-BLFH read-in. |
| BLFH-[SUB-COMPARTMENT] | Y | Requires TK-BLFH [sub-compartment] read-in and additional user/system accreditation. |
| IDITAROD | Y | Requires TK-IDIT read-in. |
| IDIT-[SUB-COMPARTMENT] | Y | Requires TK-IDIT [sub-compartment] read-in and additional user/system accreditation. |
| KANDIK | Y | Requires TK-KAND read-in. |
| KAND-[SUB-COMPARTMENT] | Y | Requires TK-KAND [sub-compartment] read-in and additional user/system accreditation. |
| **5. Special Access Program Markings** |  |  |
| SPECIAL ACCESS REQUIRED-[PROGRAM IDENTIFIER] | Y | Requires SAR-[PROGRAM-IDENTIFIER] read-in and additional user/system accreditation. |
| **6. Atomic Energy Act Information Markings** |  |  |
| RESTRICTED DATA | Y | Requires Q clearance. |
| CNWDI | Y | Requires Q access and CNWDI read-in. Note: For DoD personnel, including DoD IC elements, Q access is not required. |
| SIGMA [#] | Y | Requires Q access and SIGMA [#] read-in. |
| FRD | N | Presence of FRD does not impact ARH. Note: Foreign disclosure/release requires DOE approval. |
| DOD UCNI | Y | Requires organizational affiliation of US federal government, or US SLTT government with staff role (contractors not authorized). Note: USA country affiliation is not required. |
| DOE UCNI | Y | Requires organizational affiliation of US federal government, or US SLTT government with staff role (contractors not authorized). Note: USA country affiliation is not required. |
| TFNI | N | Presence of TFNI does not impact ARH. |
| **7. Foreign Government Information Markings** |  |  |
| FGI | N | Presence of FGI does not impact ARH. |
| FGI [LIST] | Y/N | Presence of FGI [LIST] with NATO in [LIST] requires NATO read-in. Presence of FGI [LIST] without NATO in [LIST] does not impact ARH. |
| **8. Dissemination Control Markings** |  |  |
| RSEN | N | Presence of RSEN does not impact ARH. |
| FOUO | Y | Requires organizational affiliation of US federal government, or US SLTT government. Note: USA country affiliation is not required. |
| ORCON | Y | Requires organizational affiliation of US federal government department/agency as specified by originator. |
| ORCON-USGOV | Y | Requires organizational affiliation of US Executive Branch department/agency, or US congressional intelligence committee(s). |
| IMCON | N | Presence of IMCON does not impact ARH. |
| NOFORN | Y | Requires USA country affiliation and US federal or SLTT government affiliation. |
| PROPIN | Y | Contact originator for access requirements. |
| REL TO [USA, LIST] | Y | Requires USA or [LIST] country affiliation. |
| RELIDO | N | Presence of RELIDO does not impact ARH. Note: RELIDO does not by itself indicate an affirmative FD&R decision. |
| USA/[LIST] EYES ONLY | Y | Requires USA or [LIST] country affiliation. |
| DEA SENSITIVE | Y | Requires USA country affiliation; if classified, requires clearance at or above classification; and requires organizational affiliation of US federal government or US SLTT government with staff role. |
| FISA | Y | Requires organizational affiliation of US federal government, or US SLTT government. Note: USA country affiliation is not required. |
| DISPLAY ONLY [LIST] | Y | Requires [LIST] country affiliation. Note: DISPLAY ONLY grants viewing only, not copy/duplicate/further dissemination. |
| **9. Non-Intelligence Community Dissemination Control Markings** |  |  |
| LIMDIS | Y | Requires USA country affiliation and organizational affiliation of US federal government. |
| EXDIS | Y | Requires USA country affiliation and organizational affiliation of US federal government department/agency as specified by originator. |
| NODIS | Y | Requires USA country affiliation and named-individual access. |
| SBU | Y | Requires organizational affiliation of US federal government. Note: USA country affiliation is not required. |
| SBU NOFORN | Y | Requires USA country affiliation and organizational affiliation of US federal government. |
| LES | Y | Requires clearance at level of LES information, and one of: US federal government affiliation, US SLTT government affiliation, or need-to-know. |
| LES NOFORN | Y | Requires USA country affiliation, clearance at level of LES NOFORN information, and one of: US federal government affiliation, US SLTT government affiliation, or need-to-know. |
| SSI | Y | Requires organizational affiliation of US federal government. Note: USA country affiliation is not required. |

end page 45               UNCLASSIFIED

## 8. (U) Dissemination Control Markings

(U) General Information

(U) Dissemination Controls are control markings that identify the expansion or limitation on the distribution of information. These markings are in addition to and separate from the levels of classification defined by EO 13526. (U) The Information Security Oversight (ISOO) Implementing Directive (32CFR2001, §2001.24[j][2]), identifies the DNI as the authority over external dissemination control and handling markings for intelligence and intelligence-related information. Only those DNI-authorized external dissemination control and handling markings contained in the Register may be used by IC elements to control and handle external dissemination of classified information. (U) Multiple entries may be chosen from the Dissemination Control Markings category. If multiple entries are used, list them in the order in which they appear in the Register . Use a single forward slash with no interjected space as the separator between multiple dissemination control markings.

(U) Note:   Some dissemination controls are restricted to use by certain Agencies. They are included in the Register to provide guidance on handling documents that bear them. Their inclusion in the Register does not authorize other agencies to originate these markings.

(U) The following dissemination control markings and their respective sponsor(s) are listed below in the order they appear in the Register :
- RISK SENSITIVE (NGA)
- FOR OFFICIAL USE ONLY (Various Agencies)
- ORIGINATOR CONTROLLED (DNI)
- ORIGINATOR CONTROLLED-USGOV (DNI)
- CONTROLLED IMAGERY (DNI)
- NOT RELEASABLE TO FOREIGN NATIONALS (DNI)
- CAUTION-PROPRIETARY INFORMATION INVOLVED (DNI)
- AUTHORIZED FOR RELEASE TO [USA, LIST] (DNI)
- RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL (DNI)
- USA/[country trigraphs] EYES ONLY (NSA)   Note:   NSA has been granted a control markings waiver through 9 September 2016, at which time it will expire automatically. Automated systems will be modified to reject information marked EYES ONLY beginning 01 October 2017.
- DEA SENSITIVE (DEA)
- FOREIGN INTELLIGENCE SURVEILLANCE ACT (DNI)
- DISPLAY ONLY [LIST] (DNI)

end page 131               UNCLASSIFIED

---
begin page 132               UNCLASSIFIED

(U) RISK SENSITIVE (U) Authorized Banner Line Marking Title:   RISK SENSITIVE

(U)   Authorized Banner Line Abbreviation:   RSEN

(U) Authorized Portion Mark:   RS

(U) Example Banner Line:   TOP SECRET//TK//RSEN

(U)   Example Portion Mark:   (TS//TK//RS)

(U) Marking Sponsor/Policy Basis:   NGA/National System for GEOINT (NSG)

(U) Definition:   This term is used to protect especially sensitive imaging capabilities and exploitation techniques.

(U) Further Guidance:
- NGA, Geospatial Intelligence (GEOINT) Security Classification Guide (SCG) Annex: (U) Sensitive Analytical Techniques (SAT)
- NSGM documentation when TK and RSEN are used together
- Talent Keyhole Control System Manual
- NSG GEOINT Security Classification Guide

(U) Applicability : Available for use by all agencies.

(U) Additional Marking Instructions:
- Applicable only to Top Secret or Secret information.
- See Warnings and Notices below for important guidance regarding use of SAT information.

(U) Relationship(s) to Other Markings:
- May only be used with TOP SECRET or SECRET.
- Ma y be used with TK.

(U) Precedence Rules for Banner Line Guidance:   If any portion contains RISK SENSITIVE information, the RSEN marking must appear in the banner line.

(U) Commingling Rule(s) Within a Portion:   May be combined with other information when appropriate and the RS marking must be conveyed in the portion mark.

(U) Derivative Use (i.e., re-use of information in whole or in part in intelligence products):   RSEN information may be sourced in accordance with relevant IC policy and/or procedures. See above precedence and commingling rules.

(U) Warnings and Notices  (U//FOUO)   Some TS//TK/RSEN information has been designated by the Sensitive Analytical Techniques Panel for special protection. When SATs are used in documents or graphics, it is necessary to use the appropriate warning statement. Warni ng s shall be p laced at the t op of a document and lef t j ustified.

end page 132               UNCLASSIFIED

---
begin page 133               UNCLASSIFIED

- For TS//TK//RSEN SAT information:
   - “(U//FOUO)   Warning: This document contains references to Sensitive Analytical Techniques (Sensi tive TK Imagery Information)”
- For TS//TK//RSEN SAT combined with IMCON//REL TO USA, FVEY information:
   - "(U//FOUO) Warning: This document contains references to Sensitive Analytical Techniques (Sensitive TK Imagery Information and IMCON Information). Further reuse or dissemination of this information beyond USA , AUS, CAN, GBR, or NZL requires written approval of the NGA Foreign Disclosure Officer. Contact the SAT Panel Executive Officer for queries IMCON information is not permitted on the SECRET network  ( i .e., SIPRNET )   without prior written approval by the SATP Chair."
- For TS//TK//RSEN SAT combined with NOFORN/IMCON information:
   - "(U//FOUO) NOTICE: This document contains references to Sensitive Analytical Techniques (Sensitive TK Imagery Information and IMCON Information). Further reuse or dissemination of this information beyond USA requires written approval of the NGA Foreign Disclosure Officer. Contact the SAT Panel Executive Officer for que ries MCON information is not permitted on the SECRET network (i.e., SIPRNET) without prior written approval by th e SATP Cha ir."

(U)   Notional Example Page:  TOP SECRET//TK//RSEN/REL TO USA, ACGU (TS//TK//RS/REL TO USA, ACGU) This is the portion mark for a portion that is classified TOP SECRET, contains TALENT KEYHOLE information, is handled as RISK SENSITIVE, and is authorized for release to ACGU (i.e., USA, Australia, Canada and United Kingdom). This portion must contain only US classified information that is releasable to ACGU. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//TK//RSEN/REL TO USA , A CGU  (b)(3) 10 U.S.C. 424  (b)(3) 10 U.S.C. 424

end page 133               UNCLASSIFIED

---
begin page 134               UNCLASSIFIED

(U) FOR OFFICIAL USE ONLY (U) Note: This marking will be evaluated for continued registration with the 14 November 2016 implementation of the Controlled Unclassified Information (CUI) Program.

(U)   Authorized Banner Line Marking Title:   FOR OFFICIAL USE ONLY

(U)   Authorized Banner Line Abbreviation:   FOUO

(U)   Authorized Portion Mark:   FOUO

(U)   Example Banner Line:   UNCLASSIFIED//FOUO

(U)   Example Portion Mark:   (U//FOUO)

(U)   Marking Sponsor/Policy Basis:   Various A ge ncies

(U) Definition:   Intelligence marking used for UNCLASSIFIED official government information that is withheld from public release until ap pr oved for release b y the originator.

(U)   Further Guidance:   Agency specific.

(U) Applicability:   Available for use by all agencies.

(U) Additional Marking Instructions:   Applicable only to unclassified information.

(U) Relationship(s) to Other Markings:
- May only be used with UNCLASSIFIED.
- Documents that are UNCLASSIFIED//FOR OFFICIAL USE ONLY or UNCLASSIFIED//FOUO must be portion marked.

(U) Precedence Rules for Banner Line Guidance:
- FOUO in a classified document:
   - When a classified document contains portions of FOUO information, the FOUO marking is not used in the banner line.
   - Portions of a classified document that are FOUO must be portion marked “(U//FOUO)”.
- FOUO in an unclassified document:
   - FOUO must convey in the banner line if the document is UNCLASSIFIED with FOUO marked information and no other dissemination control markings.
   - FOUO must convey in the banner line if the document is UNCLASSIFIED with only FOUO and FD&R markings. The appropriate FD&R markings must also convey in the banner line based on existing banner line roll-up rules for FD&R markings.
   - FOUO is not conveyed in the banner line if the document is UNCLASSIFIED with FOUO and other dissemination control markings, excluding any FD&R markings.

(U) Commingling Rule(s) Within a Portion:   May be combined with other information when appropriate and the FOUO marking only conveys in the portion mark based on the rules provided above for banner line precedence.

end page 134               UNCLASSIFIED

---
begin page 135               UNCLASSIFIED

(U) Derivative Use (i.e., re-use of information in whole or in part in intelligence products):   FOUO information may be sourced in accordance with relevant policy and/or procedures. See above precedence and commingling rules.

(U)   Notional Example Page:  UNCLASSIFIED//FOR OFFICIAL USE ONLY (U//FOUO) This is the portion mark for an UNCLASSIFIED FOR OFFICIAL USE ONLY portion. This portion is marked for training purposes only. UNCLASSIFIED//FOR OFFICIAL USE ONLY

end page 135               UNCLASSIFIED

---
begin page 136               UNCLASSIFIED

(U) DISSEMINATION AND EXTRACTION OF INFORMATION CONTROLLED BY ORIGINATOR

(U)   Authorized Banner Line Marking Title:   ORIGINATOR CONTROLLED

(U)   Authorized Banner Line Abbreviation:   ORCON

(U) Portion Mark:   OC

(U)   Example Banner Line:   TOP SECRET//ORCON

(U)   Example Portion Mark:   (S//OC )

(U) Marking Sponsor/Policy Basis:   DNI/National Security Act of 1947, §103 (c)(5)

(U) Definition:   The ORCON marking may be applied by originators of classified national intelligence information that meets one or more of the criteria listed in section §D.3 of Intelligence Community Policy Guidance (ICPG) 710.1,  Application of Dissemination Controls: Originator Control.   This marking allows originators to maintain knowledge, supervision, and control of the distribution of the ORCON information beyond its original dissemination. Further dissemination of ORCON information requires advance permission from the originator.

(U) Note:   Use the ORCON-USGOV marking rather than ORCON when ORCON information is pre-approved by the originator for dissemination to Executive Branch departments and agencies and for use in disseminated analytic products for congressional intelligence committees that have oversight functions and responsibilities. (See the ORCON-USGOV template for additional guidance.)

(U) Further Guidance:
- Principal Deputy Director of National Intelligence Memo, E/S 00124, dated 14 February 2008
- ICD 710
- ICPG 710.1

(U) Applicability:   Available for use by all IC agencies as appropriate.

(U) Additional Marking Instructions:
- Applicable only to classified information.
- For DAPs, originators must ensure each product includes a human and machine readable originator and dissemination list per ICPG 710.1 §E.4.b.

(U) Relationship(s) to Other Markings:
- May only be used with TOP SECRET, SECRET, or CONFIDENTIAL.
- May not be used with ORCON-USGOV in a portion mark or banner line.
- May be used with NOFORN, REL TO, DISPLAY ONLY.
- May not be used with RELIDO.

(U) Precedence Rules for Banner Line Guidance:
- The ORCON marking must always appear in the banner line if any portion contains ORCON information.
- If ORCON and ORCON-USGOV portions are in a document, ORCON takes precedence and is conveyed in the banner line.

end page 136               UNCLASSIFIED

---
begin page 137               UNCLASSIFIED

(U) Commingling Rule(s) Within a Portion:
- May be combined with other classified, non-ORCON information when appropriate (e.g., SCI, SAP, AEA, FGI, IC and Non-IC dissemination controls) and the OC marking is conveyed in the portion mark.
- If combined with non-ORCON information in a single portion, the non-ORCON information must also be handled in a manner consistent with ORCON information as provided in ICPG 710.1 (see first Note below).
- Originating agencies are responsible for determining and providing guidance to classifiers as to when it is appropriate to commingle ORCON information in the same portion with other information that has SCI/SAP, AEA, FGI, or IC and Non-IC dissemination control markings.
- If ORCON and REL TO are used in a portion marking, the originating IC element is explicitly indicating a positive release decision to the foreign recipients on original distribution. Recipients must obtain originator approval prior to further dissemination beyond the original distribution list.
- May be combined with ORCON-USGOV information in the same portion; the OC marking takes precedence and is conveyed in the portion mark.

(U) Notes:
- Originators of ORCON information must separate sources, methods, and activities content from the substantive classified national intelligence by using tearlines, write for release, or other sanitization methods in accordance with DNI policy.
- ORCON information may be shared with all recipients who have been authorized to receive the information on the dissemination list (either the original dissemination list or an originator-authorized amendment to the dissemination list) or by pre-approval, to include dissemination to congressional committees consistent with oversight functions and requirements.
- Recipients of ORCON information may further share the information without additional approval from the originator to organizations and individuals described in ICPG 710.1, §E.1.b., and in accordance with requirements for access to classified NSI set forth in EO 13526, §4.1(a).
- Requests for further sharing of ORCON information beyond the organizations and individuals described in ICPG 710.1, §E.1., to include recipients outside the IC, require advance permission of the originator.

(U) Derivative Use (i.e., re-use of information in whole or in part in intelligence products):
- The use of ORCON enables the originator to maintain knowledge, supervision, and control of the distribution of ORCON information beyond its original dissemination. Further dissemination of ORCON information requires advance permission from the originator, who must make a determination based on the mission need of the intended recipient.  o Information marked with ORCON may be incorporated in whole or in part into other briefings or products, provided the briefing or product is presented or distributed only to original recipients of the information and those organizations and individuals described in ICPG 710.1, §E.1.b.
- As described in ICPG 710.1 §E.3.a, the Principal Deputy DNI (PDDNI), in consultation with the heads of IC elements, must designate Secure Communities Of Interest (SCOI) within which relevant disseminated ORCON information originated by an IC element may be posted by any authorized recipient participating in the SCOI without additional originator approvalsor control. SCOI participants are not authorized to share ORCON material outside the SCOI with any organization that is not otherwise an authorized recipient without pre - ap pro val b y the orig inator and in accordance with the proc edures in ICPG 710.1, § E.4.a.

(U) Warnings and Notices: C ertain types of ORCON information may require the addition of a distribution or warning statement. When this occurs, the required distribution or warning statement should not repeat or re-state the ORCON restrictions, but rather provide the necessary additional direction.

end page 137               UNCLASSIFIED

---
begin page 138               UNCLASSIFIED

(U)   Notional Example Page:  TOP SECRET//ORCON/NOFORN (TS//OC/NF) This is the portion mark for a portion of information that is classified TOP SECRET, ORIGINATOR CONTROLLED, and is not releasable to foreign nationals. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//ORCON/NOFORN

end page 138               UNCLASSIFIED

---
begin page 139               UNCLASSIFIED

(U) DISSEMINATION AND EXTRACTION OF INFORMATION CONTROLLED BY ORIGINATOR-USGOV (U) Authorized Banner Line Marking Title:   ORIGINATOR CONTROLLED-USGOV

(U)   Authorized Banner Line Abbreviation:   ORCON-USGOV

(U) Authorized Portion Mark:   OC-USGOV

(U)   Example Banner Line:   SECRET//ORCON-USGOV

(U) Example Portion Mark:   (S//OC-USGOV)

(U) Marking Sponsor/Policy Basis:   DNI National Security Act of 1947, as amended

(U) Definition (Description):   The ORCON-USGOV marking is used by IC elements that originate ORCON information to communicate to recipients that the information has been pre-approved for further dissemination without originator approval to the US Government’s Executive Branch Department s and Agencies. It is also approved for use in disseminated analytic products (i.e., not unevaluated or raw intelligence) and other products or information, as determined by the originating agency in consultation with their Office of Legislative Affairs (OLA), provided to congressional Intelligence Committees that have oversight functions and responsibilities. ORCON appears in the label to ensure recipients are aware of and adhere to the processes, procedures, access and reporting requirements as set forth in EO 13526, Section 4.1 and ICPG 710.1. Dissemination to other US entities requires originator approval. Originators of ORCON-USGOV must also adhere to the marking criteria and reporting requirements for ORCON information when applying the ORCON-USGOV marking as established in ICPG 710.1. (U) When applied, the ORCON-USGOV marking does not alter the definition, processes, procedures, or pre-approval agreements associated with the ORCON marking. This marking will prevent the proliferation of non-standard markings, and distribution and warning statements. It is also communicates pre-approval for dissemination to Executive Branch agencies/departments and dissemination of analytic products to congressional Intelligence Committees. Other distribution and warning statements used to communicate other ORCON pre-approval agreements remain valid and must be applied as appropriate.

(U) Note:   USG Executive Branch Departments and Agencies must contact their OLA for guidance in determining the appropriate Congressional Intelligence Committee recipient(s).

(U) Further Guidance   (cite additional issuances):
- EO 13526
- ICD 710
- ICPG 710.1

(U) Applicability:   Available for use by all IC agencies.

(U) Additional Marking Instructions:
- Applies only to classified information.
- For DAPs, originators must ensure each product includes a human and machine readable originator and dissemination list per ICPG 710.1 §E.4.b.

(U) Relationship(s) to Other Markings:
- May only be used with TOP SECRET, SECRET, CONFIDENTIAL.

end page 139               UNCLASSIFIED

---
begin page 140               UNCLASSIFIED

- May not be used with ORCON in a portion mark or banner line.
- May be used with NOFORN, REL TO, DISPLAY ONLY.
- May not be used with RELIDO.
- May not be used with SI-G or SI-G sub-compartments.
- May not be used with HCS-O. May be used with HCS-P; may not be used with HCS-P sub-compartments.

(U) Precedence Rules for Banner Line Guidance:   When a document contains both ORCON-USGOV portions and ORCON portions, ORCON takes precedence within the banner line.

(U) Commingling Rule(s) Within a Portion :
- May be commingled within the same portion with SCI/SAP, AEA, FGI, and other dissemination controls to include foreign disclosure and release markings, if the commingled information meets one or more of the ORCON use criteria as described in ICPG 710.1 paragraph D.3 and is pre-approved for dissemination to the USGOV.   (USGOV is defined as the Executive Departments/Agencies and to Congressional Intelligence Committee(s) with oversight functions and responsibilities.)
- Originating agencies are responsible for determining and providing guidance to classifiers regarding commingling ORCON-USGOV information in the same portion with other information that has SCI/SAP, AEA, FGI or other IC and Non-IC dissemination control markings.
- If ORCON-USGOV and REL TO are used in a portion marking, the originating IC element is explicitly indicating a positive release decision to the foreign recipients on original distribution. Recipients must obtain originator approval prior to further dissemination beyond the original distribution list.
- If a portion contains both ORCON and ORCON-USGOV information, ORCON takes precedence in the portion mark.
- All roll-up rules for control markings used in conjunction with the ORCON-USGOV marking remain applicable. See specific templates for additional roll-up guidance.

(U) Notes:
- Information marked ORCON-USGOV may be shared on IC SCOIs in accordance with ICPG 710.1.
- Information bearing the ORCON-USGOV marking may be stored, transmitted, posted or retrieved from classified electronic systems, databases, repositories, networks, or web pages in IC secure collaborative environments that have the ability to limit access to users who are employed by, assigned to, or acting on behalf of an Executive Branch department/agency.
- Disseminated analytic products bearing the marking may be stored, transmitted, posted, or retrieved from classified electronic systems, databases, repositories, networks, or web pages in secure collaborative environments by Congressional Intelligence Committee(s) that have oversight functions and responsibilities (recipients must consult with their legislative affairs offices on guidance to determine appropriate oversight committees).
- Information marked ORCON-USGOV may not reside in or be transmitted over unclassified systems or unclassified sharing environments of any kind.
- Legacy information (e.g., portions extracted, reintroduced into the working environment from a resting state):  The ORCON-USGOV marking may be authorized for use on legacy ORCON information only by originators, as appropriate. Contact the originating agency for guidance.
- The ORCON-USGOV marking does not replace any legacy marking; however, it provides an alternative to the legacy practice of applying ORCON with a preapproval notice that replicates the ORCON-USGOV pre- approvals.

(U) Derivative Use: (i.e., re-use of information in whole or in part in other intelligence products):   Information bearing this marking may be extracted and used in other disseminated analytic products, provided they are presented or distributed only to original and pre-approved recipients of Executive Branch departments/agencies or Congressional Intelligence Committees. A producer must receive approval from the originating agency prior to disseminating it to other organizations outside the Executive Branch or Congressional Intelligence Committees.

end page 140               UNCLASSIFIED

---
begin page 141               UNCLASSIFIED

(U) Warnings and Notices:   There may be certain types of ORCON-USGOV information that require a distribution or warning statement in addition to the marking. When this occurs, the required distribution or warning statement should not re-state what the marking means, but rather prov ide additional direction.

(U)   Notional Example Page 1 : TOP SECRET//ORCON-USGOV/NOFORN (TS//OC-USGOV/NF) This is the portion mark for a portion that is classified TOP SECRET, is controlled as ORCON-USGOV information, and is not authorized for release to foreign nationals. These markings are for training purposes only. (C//NF) This is the portion mark for a portion that is classified CONFIDENTIAL and is not authorized for release to foreign nationals. These marking are for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//ORCON-USGOV/NOFORN

(U)   Notional Example Page 2 : TOP SECRET//ORCON/NOFORN (TS//OC-USGOV/NF) This is the portion mark for a portion that is classified TOP SECRET, is controlled as ORCON-USGOV information, and is not authorized for release to foreign nationals. These markings are for training purposes only. (S//OC/NF) This is the portion mark for a portion that is classified SECRET, is controlled as ORCON information, and is not authorized for release to foreign nationals. These markings are for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//ORCON/NOFORN

end page 141               UNCLASSIFIED

---
begin page 142               UNCLASSIFIED

(U) CONTROLLED IMAGERY (U) Authorized Banner Line Marking Title:   CONTROLLED IMAGERY

(U)   Authorized Banner Line Abbreviation:   IMCON

(U) Authorized Portion Mark:   IMC

(U)   Example Banner Line:   SECRET//CONTROLLED IMAGERY

(U) Example Portion Mark:   (S//IMC)

(U)   Marking Sponsor/Policy Basis:   DNI/National Security Act of 1947 , § 103   (c) (5)  (U//FOUO)   Definition:  Controlled Imagery or IMCON is applied to SECRET-level Sensitive Analytical Techniques (SATs) that have been approved by the Sensitive Analytical Techniques Panel (SATP). SATs are sensitive imagery signatures that can be classified, with prior SATP approval, at either the SECRET//IMCON or TOP SECRET//TK//RSEN level.

(U) Further Guidance:
- NGA , Geospatial Intelligence (GEOINT) Security Classification Guide (SCG) Annex: (U) Sensitive Analytical Techniques (SAT)

(U) Applicability:   Available for use by all IC agencies.

(U) Additional Marking Instructions:
- Applicable only to TOP SECRET and SECRET information.
- IMCON Information without NOFORN may be released to AUS, CAN, GBR, and NZL without receiving prior approval from the originating agency; when appropriate, the REL TO marking is used to communicate this explicit release determination (i.e., S//IMC/REL TO USA, AUS, CAN, GBR, NZL).

(U) Relationship(s) to Other Markings:
- May only be used with TOP SECRET or SECRET   –   see precedence and commingling rules.
- May be used with REL TO or NOFORN when appropriate; release of NOFORN requires approval by the SATP.

(U) Precedence Rules for Banner Line Guidance:
- IMCON must always appear in the banner line if any portion contains IMCON information.
- A document that contains both IMCON and NOFORN portions must be marked [class level]//IMCON/NOFORN in the banner line.

(U) Commingling Rule(s) Within a Portion:   If IMCON information is included in a portion containing additional TOP SECRET releasable to Five Eyes information, the paragraph would be marked as (TS//IMC/REL TO USA, AUS, CAN, GBR, NZL). The overall classification level would be TOP SECRET//IMCON/[Explicit FD&R].

(U) Notes:
- IMCON information is not releasable to third parties without specific approval from the originating agency and the SATP.  (b)(3) 50 U.S.C. 3024i

end page 142               UNCLASSIFIED

---
begin page 143               UNCLASSIFIED

- Information bearing (S//IMC/REL TO USA, AUS, CAN, GBR, NZL) at the beginning of a paragraph may be disseminated to those countries without receiving prior approval from the originating agency. Dissemination to other entities is prohibited without the prior written approval of the originating agency and the SATP.
- This information may be used freely in Community and Command databases and may be disseminated to US military units and Intelligence Community agencies. However, products containing IMCON information are not pe rmitted on SECRET networks   ( SIP RNet)   without prio r written approval b y the SATP Executive Officer,

(U) Warnings and Notices:
- (U) Although the legacy DCID 6/6 indicated that the IMCON notice was not required beyond 1 April 2002, the Im ag er y Polic y and Security Committee (IP SCO M)   ap prov ed its continued use indefinitel y . For additional information on releasability and NOFORN issues, please contact the SATP Chair ,
- (U) Imagery and/or text reporting bearing the IMCON restriction requires one of the following notices, which shall be placed at the top of the document and left justified:
- For REL FVEY:  (U//FOUO)   NOTICE : This document contains references to Sensitive Analytical Techniques (IMCON Information) . Further reuse or dissemination of this information be yond USA, AUS , CAN, GBR, or NZL requ ires written ap pro val of the SATP. Contact the SATP Executive Officer for qu eries IMCON information is not permitted on the SECRET network (i.e., SIPRNET).
- For NOFORN:  (U//FOUO )   NOTICE : T hi s do c um en t c on ta i ns r ef eren c es to S e ns i t i v e A na l y t i c al T ec hn i qu es  (IMCON Information). Further reuse or dissemination of this information beyond USA requires written ap prov al of the NGA Forei gn Disclosure Officer. Contact the SAT Panel Executive Officer for queries IMCON information is not permitted on the SECRET network   ( i.e., SIPRNET)   without prio r written approval b y the S AT Panel Ch air.

(U) Derivative Use (i.e., re-use of information in whole or in part in intelligence products):   IMCON information may be sourced in accordance with relevant IC policy and/or procedures. See above precedence and commingling rules.

(U)   Notional Example Page 1:  SECRET//IMCON/REL TO USA, AUS, CAN, GBR, NZL  [ Insert IMCON Notice ]  (S//IMC/REL TO USA, AUS, CAN, GBR, NZL) This is the portion mark for a portion that is classified SECRET CONTROLLED IMAGERY, and is authorized for release to Australia (AUS), Canada (CAN), United Kingdom (GBR), and New Zealand (NZL). This portion must contain only US classified information that is releasable to AUS, CAN, GBR, and NZL. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//IMCON/REL TO USA, AUS, CAN, GBR, NZL  (b)(3) 10 U.S.C. 424  (b)(3) 10 U.S.C. 424  (b)(3) 10 U.S.C. 424  (b)(3) 10 U.S.C. 424

end page 143               UNCLASSIFIED

---
begin page 144               UNCLASSIFIED

(U)   Notional Example Page 2:  TOP SECRET//IMCON/NOFORN  [ Insert IMCON Notice ]  (S//IMC/REL TO USA, AUS, CAN, GBR, NZL) This is the portion mark for a portion that is classified SECRET CONTROLLED IMAGERY and is authorized for release to Australia (AUS), Canada (CAN), United Kingdom (GBR), and New Zealand (NZL). This portion must contain only US classified information that is releasable to AUS, CAN, GBR, and NZL. This portion is marked for training purposes only. (TS//NF) This is the portion mark for a portion that is classified TOP SECRET and not releasable to foreign nationals. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//IMCON/NOFORN

end page 144               UNCLASSIFIED

---
begin page 145               UNCLASSIFIED

(U) NOT RELEASABLE TO FOREIGN NATIONALS (U) Authorized Banner Line Marking Title:   NOT RELEASABLE TO FOREIGN NATIONALS

(U) Authorized Banner Line Abbreviation:   NOFORN

(U)   Authorized Portion Mark:   NF

(U)   Example Banner Line:   TOP SECRET//NOFORN

(U) Example Portion Mark:   (S//NF)

(U) Marking Sponsor/Policy Basis:   DNI/National Security Act of 1947, as amended, §103 (c)(5)

(U) Definition:   NOFORN is an explicit foreign release marking used to indicate the information may not be released in any form to foreign governments, foreign nationals, foreign organizations, or non-US citizens without permission of the orig inator and in accordance wit h p rovisions of, ICD 403, NDP-1, and im pl ementation guidance in this document.

(U) Further Guidance:
- IRPTA 2004
- EO 13526
- EO 12333, as amended
- ICPG 403.1
- ICD 710
- ICPG 710.2/403.5
- NDP-1
- Specific DNI CONOPS or other policy issuances specific to US support to ensure proper handling requirements are met

(U) Applicability:   Available for use by all IC agencies.

(U) Additional Marking Instructions:   Applicable to unclassified and classified information.

(U) Relationship(s) to Other Markings:
- May be used with TOP SECRET, SECRET, CONFIDENTIAL or UNCLASSIFIED.
- Cannot be used with REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY.  Note:   NSA is the only agency granted a control markings waiver for the continued use of the EYES ONLY marking through 9 September 2016, at which time it will expire automatically. Automated systems will be modified to reject information marked EYES ONLY beginning 01 October 2017.

(U) Precedence Rules for Banner Line Guidance:   Refer to Section D.2., Table 3 FD&R Markings Precedence Rules for Banner Line Roll-Up for guidance.

(U) Commingling Rule(s) Within a Portion:   May be combined with other caveated information when appropriate and the NF marking is conveyed in the portion mark.

(U) Notes :
- NOFORN is the most restrictive foreign disclosure and release marking.
- Unclassified information and unclassified information with dissemination controls may be marked explicitly with NOFORN at the portion and banner level as circumstances warrant. Ex p licit forei gn disclosure and release

end page 145               UNCLASSIFIED

---
begin page 146               UNCLASSIFIED

 markings are not required on unclassified information. Follow internal agency procedures for the use of NOFORN with unclassified information.

(U) Derivative Use (i.e., re-use of information in whole or in part in intelligence products):   NOFORN information may be sourced in accordance with relevant IC policy and/or procedures. See above precedence and commingling rules.

(U) Notional Example Page 1:  TOP SECRET//NOFORN (TS//NF) This is the portion mark for a portion that is classified TOP SECRET not releasable to foreign nationals. This portion is marked for training purposes only. (S//REL TO USA, JPN) This is the portion mark for a portion that is classified SECRET and is authorized for release to Japan (JPN). This portion must contain only US classified information that is releasable to JPN. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//NOFORN

(U)   Notional Example Page 2:  SECRET//NOFORN (S//REL TO USA, JPN) This is the portion mark for a portion that is classified SECRET and is authorized for release to Japan (JPN). This portion must contain only US classified information that is releasable to JPN. This portion is marked for training purposes only. (C//RELIDO) This is the portion mark for a portion that is classified CONFIDENTIAL and the originator has explicitly deferred the foreign disclosure and release determination to a SFDRA. This portion is marked for training purposes only.

(U) Note:   Documents containing multiple portions with different foreign disclosure or release markings must be marked overall with the most protective marking, in this case NOFORN is appropriate because not all portions are marked releasable to JPN.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN

end page 146               UNCLASSIFIED

---
begin page 147               UNCLASSIFIED

(U) Notional Example Page 3:  SECRET//NOFORN//LES (S//REL TO USA, JPN) This is the portion mark for a portion that is classified SECRET and is authorized for release to Japan (JPN). This portion must contain only US classified information that is releasable to JPN. This portion is marked for training purposes only. (U//LES-NF) This is the portion mark for a portion that is UNCLASSIFIED and contains LAW ENFORCEMENT SENSITIVE information not releasable to foreign nationals. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN//LES

end page 147               UNCLASSIFIED

---
begin page 148               UNCLASSIFIED

(U) CAUTION-PROPRIETARY INFORMATION INVOLVED

(U)   Authorized Banner Line Marking Title:   CAUTION-PROPRIETARY INFORMATION INVOLVED

(U)   Authorized Banner Line Abbreviation:   PROPIN

(U)   Authorized Portion Mark:   PR

(U)   Example Banner Line:   CONFIDENTIAL//PROPIN/[ Ex pl icit FD& R]

(U)   Example Portion Mark:   (S//PR /[E x p licit FD& R])

(U)   Marking Sponsor/Policy Basis:   DNI/ 18 USC 1905

(U) Definition:   Marking to identify information provided by a commercial firm or private source under an express or implied understanding that the information will be protected as a proprietary trade secret or proprietary data believed to have actual or potential value. This marking may be used on government proprietary information only when the government proprietary information can provide a contractor(s) an unfair advantage, such as US Government budget or financial information.

(U) Applicability:   Available for use by all IC elements.

(U) Additional Marking Instructions:   Applicable to unclassified and classified information.

(U) Relationship(s) to Other Markings:
- May be used with TOP SECRET, SECRET, CONFIDENTIAL or UNCLASSIFIED.

(U) Precedence Rules for Banner Line Guidance:
- The PROPIN marking must always appear in the banner line if any portion contains PROPIN information.
- When a document contains PROPIN and FOUO portions; only PROPIN appears in the banner line. PROPIN takes precedence over FOUO in the banner line.

(U) Commingling Rule(s) Within a Portion:   May be combined with other information when appropriate and the PR marking is conveyed in the portion mark.

(U) Notes:
- Must not be disseminated outside the federal government in any form without the express permission of the originator of the intelligence and provider of the proprietary information.
- Precludes dissemination to contractors irrespective of their status to or within the US Government without the authorization of the originator of the intelligence and provider of the information.

(U) Derivative use (i.e., re-use of information in whole or in part in intelligence products):   PROPIN information may be sourced in accordance with relevant IC policy and/or procedures. See above precedence and commingling rules.

end page 148               UNCLASSIFIED

---
begin page 149               UNCLASSIFIED

(U) Notional Example Page:  SECRET//NOFORN/PROPIN (S//NF/PR) This is the portion mark for a portion that is classified SECRET CAUTION-PROPRIETARY INFORMATION INVOLVED and not releasable to foreign nationals. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN/PROPIN

end page 149               UNCLASSIFIED

---
begin page 150               UNCLASSIFIED

(U) AUTHORIZED FOR RELEASE TO (U) Authorized Banner Line Marking Title:   AUTHORIZED FOR RELEASE TO [USA, LIST]

(U) Authorized Banner Line Abbreviation:   REL TO [USA, LIST]

(U) Authorized Portion Mark   (when the portion's country trigraphs and/or tetragraph list is different from the banner line REL TO marking ) :  REL TO [USA, LIST]

(U) Authorized Portion Mark   (when the portion's country trigraphs and/or tetragraph list is the same as the banner line REL TO marking ) :  REL

(U) Example Banner Line:   TOP SECRET//REL TO USA, EGY, ISR

(U) Example Portion Mark:   (S//REL TO USA, TEYE)  or  (S//REL) when the portion's country trigraphs and/or tetragraph list is the same as the banner line REL TO marking

(U)   Marking Sponsor/Policy Basis:   DNI/National Security Act of 1947, as amended, § 103   (c) (5)

(U) Definition:   REL TO is an explicit foreign disclosure and release marking to indicate the information has been predetermined by the originator to be releasable or has been released to the foreign country(ies)/international organization(s) indicated through established foreign disclosure procedures and channels and implementation guidance in this document. It is NOFORN to all other foreign country(ies)/international organization(s)   not indicated in the REL TO marking. Per ICD 403, release is defined as the provision of classified intelligence, in writing or in any other medium, to authorized forei gn reci p ients for retention.

(U) Further Guidance:
- IRPTA 2004
- EO 13526
- EO 12333, as amended
- Updates to policy addressing the rescinded DCID 6/6,§IXF and DCID 6/7 are pending ODNI ICD 403
- ICPG 403.1
- ICD 710
- ICPG 710.2/403.5
- NDP-1
- Specific DNI CONOPS or other policy issuances specific to US support to ensure proper handling requirements are met

(U) Applicability:   Available for use by all IC elements.

end page 150               UNCLASSIFIED

---
begin page 151               UNCLASSIFIED

(U) Additional Marking Instructions:
- Applicable to unclassified and classified information.
- “[USA, LIST]” pertains to the string that contains “USA” followed by one or more Register, Annex B trigraph country code(s) or Register, Annex A tetragraph code(s), or Manual, Appendix B NATO/NAC markings used with the REL TO marking.   “USA” must always appear first whenever the REL TO string is used to communicate release decisions either by the US or a Non-US entity.
- After “USA”, li st the required one or more trigraph country codes in alphabetical order followed by tetragraph codes listed in alphabetical order. Each code is separated by a comma and a space.
- “REL TO USA” or “REL USA” (i.e., there is not at least one country trigraph code or tetragraph code following the USA code), is not an authorized marking and is not allowed on US intelligence information.
- Country trigraph codes/tetragraph codes are followed by a single forward slash if more dissemination controls follow or a double forward slash if non-IC dissemination control markings follow. If no markings follow, then no text or separating characters follow the last country code/tetragraph code.

(U) Relationship(s) to Other Markings:
- May be used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED.
- Cannot be used with NOFORN or EYES ONLY ( Note:   The EYES ONLY marking is only authorized for use by NSA through a markings waiver; it will no longer be an IC authorized marking after 01 October 2017.)
- May be used with RELIDO.
- May be used with DISPLAY ONLY.
- For use with RD and FRD, see RD and FRD sections of this manual

(U) Precedence Rules for Banner Line Guidance:   Refer to Section D.2., Table 3 FD&R Markings Precedence Rules for Banner Line Roll-Up for guidance.

(U) Commingling Rule(s) Within a Portion:   Information marked with a REL TO caveat may be combined with other caveated information when appropriate; however, the REL TO marking will convey in the portion mark only if all information in that portion is releasable to the same “[LIST]” value(s).

(U) Notes:
- Further foreign dissemination of the material (in any form) is authorized only after obtaining permission from the originator.
- ICD 403, and NDP-1. Follow internal agency procedures for obtaining foreign disclosure and release guidance on classified information.
- Unclassified information and unclassified information with dissemination controls may be explicitly marked with REL TO at the portion and banner level as circumstances warrant. Explicit foreign disclosure and release markings are not required on unclassified information. Follow internal agency procedures for the use of REL TO with unclassified information.

(U) Derivative Use (i.e., re-use of information in whole or in part in intelligence products):   May be sourced when appropriate provided that:
- When extracting a portion marked with the “REL” abbreviation (e.g., S//REL) from a source document, carry forward the trigraph/tetragraph code(s) listed in the source document’s banner line REL TO marking to the new portion mark. (Example 1)
- When ex tracting a portion marked with the “REL TO [LIST]” from a source document, carry forward the trigraph/tetragraph code(s) listed in the source document or taken from the instructions in the appropriate classification guide to the new portion mark. See above precedence and commingling rules.

end page 151               UNCLASSIFIED

---
begin page 152               UNCLASSIFIED

(U)   Notional Example Page 1:  TOP SECRET//REL TO USA, EGY, ISR (TS//REL) This is the portion mark for a portion that is classified TOP SECRET authorized for release to Egypt (EGY) and Israel (ISR) (i.e., the same as the banner line). This portion is marked for training purposes only.

(U) Note:   When extracting a portion marked from a source document with the REL TO marking abbreviation “REL”,  carry forward the trigraph/tetragraph code(s) listed in the source document ’s banner line REL TO marking to the new portion mark, e.g., (TS//REL TO USA, EGY, ISR).

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//REL TO USA, EGY, ISR

(U)   Notional Example Page 2:  SECRET//NOFORN (S//REL TO USA, FVEY) This is the portion mark for a portion that is classified SECRET, AUTHORIZED FOR RELEASE TO FVEY (i.e., USA, Australia, Canada, New Zealand and United Kingdom) and that the originator has determined is releasable by an information disclosure official. This portion is marked for training purposes only. (C) This is the portion mark for a portion that is classified CONFIDENTIAL. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN

(U) Notional Example Page 3:  SECRET//NOFORN (S//REL TO USA, FVEY) This is the portion mark for a portion that is classified SECRET, AUTHORIZED FOR RELEASE TO FVEY (i.e., USA, Australia, Canada, New Zealand and United Kingdom) and that the originator has determined is releasable by an information disclosure official. This portion is marked for training purposes only. (C//REL TO USA, ISAF) This is the portion mark for a portion that is classified CONFIDENTIAL, AUTHORIZED FOR RELEASE TO ISAF. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN

end page 152               UNCLASSIFIED

---
begin page 153               UNCLASSIFIED

(U)   Notional Example Page 4:  SECRET//NOFORN (S//REL TO USA, AUS) This is the portion mark for a portion that is classified SECRET authorized for release to Australia (AUS). This portion is marked for training purposes only. (C//RELIDO) This is the portion mark for a portion that is classified CONFIDENTIAL and the originator has explicitly deferred the foreign disclosure and release determination to a SFDRA. This portion is marked for training purposes only.

(U) Note:   Documents containing multiple portions with different foreign disclosure or release markings must be marked overall with the most protective marking.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN

(U)   Notional Example Page 5:  SECRET//REL TO USA, NZL (S//REL TO USA, JPN, NZL) This is the portion mark for a portion that is classified SECRET authorized for release to Japan (JPN) and New Zealand (NZL). This portion is marked for training purposes only. (S//REL) This is the portion mark for a portion that is classified SECRET and authorized for release to NZL. The abbreviated “REL” portion mark may be used when a porti on is releasable to exactly the same list of countries/organizations as are listed in the banner line REL TO marking”. This portion is marked for training purposes only.

(U) Note:   When extracting a portion marked with the “REL” abbreviation from a source document, carry forward the trigraph/tetragraph codes listed in the source document’ banner line REL TO marking to the new portion mark,  e.g., (S//REL TO USA, NZL).

(U) Note:   REL TO with an overlap in the country lists, roll-ups to the most restrictive list. New Zealand appears in the banner line because this country appears in all portions.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//REL TO USA , N ZL

end page 153               UNCLASSIFIED

---
begin page 154               UNCLASSIFIED

(U) RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL (U) Authorized Banner Line Marking Title:   RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL

(U) Authorized Banner Line Abbreviation:   RELIDO

(U)   Authorized Portion Mark:   RELIDO

(U)   Example Banner Line:   TOP SECRET//TK//RELIDO

(U)   Example Portion Mark:   (S//REL TO USA , AUS/RELIDO )

(U) Marking Sponsor/Policy Basis:   DNI/National Security Act of 1947, as amended, §103 (c)(5)

(U) Definition:   RELIDO is a permissive foreign disclosure and release marking used on information to indicate that the originator has authorized a Senior Foreign Disclosure and Release Authority (SFDRA) to make further sharing decisions for uncaveated intelligence material (intelligence with no restrictive dissemination controls) in accordance with the existi ng pr ocedures , g uidelines, and im pl ementation guidance in this document.

(U) Further Guidance:
- ICD 403
- ICD 401.1
- ICD 710
- ICPG 710.2/403.5

(U) Applicability:   Available for use by all IC elements.

(U) Additional Marking Instructions:   Applicable to unclassified and classified information.

(U) Relationship(s) to Other Markings:
- May be used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED.
- May be used independently or with REL TO.
- Cannot be used with NOFORN or DISPLAY ONLY.

(U) Precedence Rules for Banner Line Guidance:   Refer to Section D.2., Table 3 FD&R Markings Precedence Rules for Banner Line Roll-Up for guidance.

(U) Commingling Rule(s) Within a Portion:
- May be combined with other caveated information when appropriate; however, the RELIDO marking is conveyed in the portion mark only when all combined information carries a RELIDO decision.

(U) Notes:
- Authorizes only SFDRAs to make further sharing decisions without consulting the originator.
- Unclassified information and unclassified information with dissemination controls may be explicitly marked with RELIDO at the portion and banner level as circumstances warrant. Explicit foreign disclosure and release markings are not required on unclassified information. Follow internal agency procedures for the use of RELIDO with unclassified information.

end page 154               UNCLASSIFIED

---
begin page 155               UNCLASSIFIED

(U) Derivative Use (i.e., re-use of information in whole or in part in intelligence products):   RELIDO information may be sourced in accordance with relevant IC policy and/or procedures. See commingling and precedence rules above.

(U) Notional Example Page 1:  TOP SECRET//TK//RELIDO (TS//TK//RELIDO) This is the portion mark for a portion that is classified TOP SECRET, contains TALENT KEYHOLE information that the originator has explicitly deferred the foreign disclosure and release determination to a SFDRA. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//TK//RELIDO

(U)   Notional Example Page 2:  SECRET//RELIDO (S//RELIDO) This is the portion mark for a portion that is classified SECRET and the originator has explicitly deferred the foreign disclosure and release determination to a SFDRA. This portion is marked for training purposes only. (S//REL TO USA, AUS, CAN/RELIDO) This is the portion mark for a portion that is classified SECRET for which the originator has made a release decision for the listed countries and for which the originator has further determined is releasable by an information disclosure official. This portion is marked for training purposes only.

(U) Note:   Redaction of the “REL TO” designators by the SFDRA may be required before the material is released in accordance with existing guidance.

(U) Note:   The RELIDO marking is carried forward to the banner line because it appears on all portions. REL TO cannot be applied to the overall classification of the document, because a positive release decision to AUS and CAN has not been made for portion 1. NOFORN would not be added because RELIDO removes the limited exception to NOFORN in portions 1 and 2. The overall classification still allows further release by an SFDRA in accordance with existing sharing guidelines.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//RELIDO

end page 155               UNCLASSIFIED

---
begin page 156               UNCLASSIFIED

(U)   Notional Example Page 3:  SECRET//NOFORN (S//RELIDO) This is the portion mark for a portion that is classified SECRET and the originator has explicitly deferred the foreign disclosure and release determination to a SFDRA. This permissive dissemination control marking has exactly the same effect as uncaveated SECRET for future sharing decisions by an SFDRA, but explicitly states that an SFDRA may make further sharing decisions in accordance with the existing procedures for uncaveated intelligence material (e.g., intelligence without restrictive dissemination controls). This portion is marked for training purposes only. (S//REL TO USA, AUS, CAN) This is the portion mark for a portion that is classified SECRET that the originator has made a release decision for the listed countries. This portion is marked for training purposes only.

(U) Note:   NOFORN must be added to the banner line, because it is the most protective marking. All portions must be marked as RELIDO for the RELIDO marking to appear in the banner line.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN

end page 156               UNCLASSIFIED

---
begin page 157               UNCLASSIFIED

(U) USA/[LIST] EYES ONLY (U) Authorized Banner Line Marking Title:   USA/[LIST] EYES ONLY

(U)   Authorized Banner Line Abbreviation:   None

(U) Authorized Portion Mark:   EYES Note:   Second Party countries do not need to be listed unless they are different from the countries listed in the EYES ONLY statement within the header and footer. If countries are different, the portion mark has the same format as the page marking listed above (i.e., USA/[country trigraphs] EYES ONLY ) .

(U) Example Banner Line:   SECRET//USA/CAN/GBR EYES ONLY

(U) Example Portion Mark:   (TS//EYES)

(U)   Marking Sponsor/Policy Basis:   NSA/CSS Classification Manual 1-52

(U) Definition:   EYES ONLY is a foreign disclosure and release marking for use only on electrical SIGINT reporting.

(U) Applicability:   NSA only (See note below)

(U) Additional Marking Instructions:   Applicable to only classified information.

(U) Relationship(s) to Other Markings:
- May only be used with TOP SECRET, SECRET and CONFIDENTIAL.
- Used with one or more Second Party countries (Australia, Canada, Great Britain New Zealand), See Register, Annex B for trigraph country codes .
- Country trigraph codes are separated by single forward slashes (USA first, others in alphabetical order).
- Cannot be used with NOFORN or REL TO.
- Can be used with RELIDO.

(U) Precedence Rules for Banner Line Guidance:   Refer to Section D.2., Table 3 FD&R Markings Precedence Rules for Banner Line Roll-Up .

(U) Notes:   The DNI has approved an extension to the waiver previously set to expire on 01 October 2016 for NSA to continue to use this marking through 01 October 2017 at which time the waiver will expire automatically. All IC systems that mark and disseminate intelligence information must be modified to reject information with the EYES ONLY markings beginning 1 October 2017.

(U) Derivative use (i.e., re-use of information in whole or in part in intelligence products):   When extracting EYES ONLY po rtions from SIGINT reporting , convert the EYES ONLY po rtion marks to REL TO.

end page 157               UNCLASSIFIED

---
begin page 158               UNCLASSIFIED

(U)   Notional Example Page:  TOP SECRET//USA/CAN/GBR EYES ONLY (TS//EYES) This is the portion mark for a portion that is classified TOP SECRET USA/CAN/GBR EYES ONLY. This portion is marked for training purposes only.

(U) Note:   When extracting “EYES” abbreviated portions from SIGINT reporting convert the “EYES” po rtion marks to REL TO and carry forward the trigraph/tetragraph codes listed in the source document banner line to the new portion mark.

(U) Note:   The EYES ONLY marking is only authorized for use by NSA systems producing SIGINT reporting, it will no longer be an IC authorized marking after 9 September 2016.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//USA/CAN/GBR EYES ONLY

end page 158               UNCLASSIFIED

---
begin page 159               UNCLASSIFIED

(U) DEA SENSITIVE (U) Note: This marking will be evaluated for continued registration with the 14 November 2016 implementation of the Controlled Unclassified Information (CUI) Program.

(U)   Authorized Banner Line Marking Title:   DEA SENSITIVE

(U)   Authorized Banner Line Abbreviation:   None

(U)   Authorized Portion Mark:   DSEN

(U)   Example Banner Line:   UNCLASSIFIED//DEA SENSITIVE

(U)   Example Portion Mark:   ( U//DSEN)

(U)   Example Banner Line:   SECRET//NOFORN/DEA SENSITIVE

(U)   Marking Sponsor/Policy Basis:   DEA/Planni ng and Ins pe ction Manual, Cha pte r 86

(U) Definition:   Unclassified information originated by DEA that requires protection against unauthorized disclosure to protec t sources and methods of investigative activity , evidence, and the inte grity of pretr ial investi ga tive re po rts.

(U) Further Guidance:   Control and Decontrol of DEA Sensitive Information Policy

(U) Applicability:   DoJ and DoD.

(U) Additional Marking Instructions:   Applicable only to unclassified information.

(U) Relationship(s) to Other Markings:
- May be used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED   –   see precedence and commingling rules.

(U) Precedence Rules for Banner Line Guidance:   If DSEN is contained in any portion of a document (classified or unclassified), it must appear in the banner line. When comingled with FOUO, DSEN supersedes FOUO in the banner line role-up.

(U) Commingling Rule(s) Within a Portion:
- When a portion contains both DSEN and FOUO information, DSEN supersedes FOUO in the portion mark.
- DSEN information may be commingled in the same portion with non-DSEN information (classified or unclassified) when appropriate and if the document includes source reference citations in accordance with ICD 206, Sourcing Requirements for Disseminated Analytic Products , dated 17 October 2007.
   - The DSEN marking is conveyed in the portion mark.
   - The DSEN information must be identified in the source reference citations as endnotes keyed to the relevant DSEN information in the document.
- If the document does not include source reference citations in accordance with ICD 206, the DSEN portions must be segregated from all non-DSEN portions.

(U) Derivative Use (i.e., re-use of information in whole or in part in intelligence products):
- DSEN information may be sourced in accordance with relevant policy and/or procedures. See above prec edence and commi ng li ng rules.

end page 159               UNCLASSIFIED

---
begin page 160               UNCLASSIFIED

- Foreign disclosure and release determinations require prior approval of the originating agency. Until originator approval is obtained, mark DSEN portions as NOFORN when an FD&R marking is required as described in Section B, paragraph 3 of this document.

(U) Warnings and Notices:
- Distribution of DEA SENSITIVE information, material, and media outside DEA is prohibited except where there is a specific need for the information to be referred to other agencies for their information or action.
- The following notation will be typed, labeled or stamped on each DEA SENSITIVE document or media sent to another agency:   “ (U) This document is the property of the DEA and may be distributed within the Federal Government (and its contractors), and to US intelligence, law enforcement, and public safety or protection officials with a need to know. Distribution beyond these entities without DEA authorization is strictly prohibited. Precautions should be taken to ensure this information is stored and/or destroyed in a manner that precludes unauthorized access. The use of information in this report is pre-approved for US government Intelligence Community products, including finished analytic products distributed to US Executive Branch departments/agencies. Cited portions must carry the same classification and controls, and readers of this report must hold all appropriate clearances. The information in this report may not be used in legal proceedings, for operational or intelligence collection activities, shared with foreign persons or agencies, entered into non-DEA databases for operational purposes or reproduced in additional formats unless express permission is granted by the DEA based on a written request

(U)   Notional Example Page 1:  UNCLASSIFIED//DEA SENSITIVE  [ Insert DSEN Warning ]  (U//DSEN) This is the portion mark for a portion that is classified UNCLASSIFIED DEA SENSITIVE. This portion is marked for training purposes only. UNCLASSIFIED//DEA SENSITIVE

(U)   Notional Example Page 2:  SECRET//NOFORN/DEA SENSITIVE  [ Insert DSEN Warning ]  (U//DSEN) This is the portion mark for a portion that is classified UNCLASSIFIED DEA SENSITIVE. This portion is marked for training purposes only. (S//NF/DSEN) This is the portion mark for a portion that is classified SECRET DEA SENSITIVE and not releasable to foreign nationals. This document must include source reference citations as endnotes keyed to the relevant DSEN information, as required by ICD 206. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN/DEA SENSITIVE  (b)(3) 50 U.S.C. 3024i

end page 160               UNCLASSIFIED

---
begin page 161               UNCLASSIFIED

(U) FOREIGN INTELLIGENCE SURVEILLANCE ACT (U) Authorized Banner Line Marking Title:   FOREIGN INTELLIGENCE SURVEILLANCE ACT

(U)   Authorized Banner Line Abbreviation:   FISA

(U) Authorized Portion Mark:   FISA

(U)   Example Banner Line:   TOP SECRET//FISA

(U) Example Portion Mark:   (TS//FISA)

(U) Marking Sponsor/Policy Basis:   DNI/US Code Title 50, Chapter 36

(U) Definition:   The Foreign Intelligence Surveillance Act (FISA) of 1978, as amended, prescribes procedures for the physical and electronic surveillance and collection of "foreign intelligence information" between or among "foreign po wers" on territor y under United States control. The marking denotes the pres ence of FISA material.

(U) Further guidance:
- The FISA statute provides that information collected pursuant to the statute "may not be disclosed for law enforcement purposes unless the disclosure is accompanied by a statement that such information, or any information derived there from, may be used in a criminal proceeding only with advance authorization of the Attorney General" (50 USC 1806, 1825, 1845).
- The statement required by the FISA statute is commonly referred to as a "FISA Warning."
- Contact orig inati ng agency or local security/legal office for specific guidance.

(U) Applicability:   Agency specific.

(U) Additional Marking Instructions:
- Applicable to unclassified and classified information.
- This is an informational marking only to highlight FISA content and does not eliminate or alter the requirement to carry a FISA warning as required by law or organizational procedures.

(U) Relationship(s) to Other Markings:
- May be used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED.

(U) Precedence Rules for Banner Line Guidance:   If the FISA marking is contained in any portion of a document (classified or unclassified) it must appear in the banner line.

(U) Commingling Rule(s) Within a Portion:   May be combined with other unclassified or classified caveated information when appropriate and the FISA marking must be conveyed in the portion mark.

(U) Derivative Use (i.e., re-use of information in whole or in part in intelligence products):   FISA marked information may be sourced in accordance with relevant policy and/or procedures. See above precedence and commingling rules.

(U) Warnings and Notices:   Applicable FISA Warning(s) are to be collocated with the FISA information within the body of the document; however, due to formatting constraints of some electronically generated documents, the FISA Warning may appear in the header or footer of the document.

end page 161               UNCLASSIFIED

---
begin page 162               UNCLASSIFIED

(U)   Notional Example Page:  TOP SECRET//NOFORN/FISA  [ Insert Applicable FISA Warning ]  (TS//NF/FISA) This is the portion mark for TOP SECRET FOREIGN INTELLIGENCE SURVEILLANCE ACT information that is not releasable to foreign nationals. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. TOP SECRET//NOFORN/FISA

end page 162               UNCLASSIFIED

---
begin page 163               UNCLASSIFIED

(U) DISPLAY ONLY (U) Authorized Banner Line Marking Title:   DISPLAY ONLY [LIST]

(U)   Authorized Banner Line Abbreviation:   None

(U)   Authorized Portion Mark:   DISPLAY ONLY   [ LIST ]

(U)   Example Banner Line:   SECRET//DISPLAY ONLY IRQ

(U) Example Portion Mark:   (S//DISPLAY ONLY IRQ)

(U)   Example Banner Line with Mult iple Co untries:   CONFIDENTIAL//DISPLAY ONLY AFG, IRQ

(U) Example Portion Mark with Multiple Countries:   (C//DISPLAY ONLY AFG, IRQ)

(U) Marking Sponsor/Policy Basis:   DNI National Security Act of 1947, as amended

(U) Definition (Description):   This marking indicates the information is authorized for disclosure without providing a physical copy for retention, regardless of medium to the foreign country(ies)/international organization(s) indicated, through established foreign disclosure procedures and channels, and implementation guidance in this document. Per ICD 403 , disclosure is defined as displaying or revealing classified intelligence whether orally, in writing, or in any other medium to an authorized foreign recipient without providing the foreign recipient a copy of such information for retention.

(U) Further Guidance :
- IRPTA 2004
- EO 13526
- EO 12333, as amended
- ICD 403.1
- ICD 710
- ICPG 710.2/403.5
- Specific DNI CONOPS or other policy issuances specific to US support to ensure proper handling requirements are met

(U) Applicability:   Available for use by all IC agencies.

(U) Additional Marking Instructions:
- Applicable to unclassified and classified information.
- “[LIST]” pertains to the Annex B country trigraph code(s) or Annex A tetragraph code(s), or Manual, Appendix B NATO/NAC markings used with the DISPLAY ONLY marking. Country codes are listed alphabetically followed by tetragraph codes in alphabetical order. Multiple codes must be separated by commas with an interjected space. Authorized codes are provided in the Register Annexes.

(U) Relationship(s) to Other Markings:
- May be used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED.
- May not be used with any other dissemination control marking in the portion and banner line, unless consistent with IC directives and established intelligence sharing arrangements and procedures.   For example, DNI policy may authorize the use of REL TO in conjunction with DISPLAY ONLY under certain circumstances.
- Cannot be used with RELIDO or NOFORN.

end page 163               UNCLASSIFIED

---
begin page 164               UNCLASSIFIED

(U) Precedence Rules for Banner Line Guidance:   Refer to Section D.2., Table 3 FD&R Markings Precedence Rules for Banner Line Roll-Up .

(U) Commingling Rule(s) Within a Portion : DISPLAY ONLY can be used in conjunction with REL TO when all information within the portion has been reviewed through the originator’s foreign disclosure channels and approved for disclosure and release to separate Register, Annex B trigraph country code(s) or Register, Annex A tetragraph code(s).

(U) Notes:
- Classified intelligence marked with DISPLAY ONLY is eligible for disclosure (not release) to the one or more Register, Annex B trigraph country code(s) or Register, Annex A tetragraph code(s)   consistent with appropriate Executive Orders and IC directives/guidelines pertaining to the disclosure and release of classified intelligence information and in accordance with established international arrangements and appropriate foreign disclosure approval processes and procedures.
- Classified intelligence marked DISPLAY ONLY may not be further disclosed beyond its original authorized intended use without prior approval of the originator and consistent with IC directives/guidelines and established intelligence sharing arrangements and procedures.
- Classified intelligence marked DISPLAY ONLY must remain under US control and follow specified US control, handling, and storage procedures for classified information at all times.
- Unclassified information and unclassified information with dissemination controls may be explicitly marked with DISPLAY ONLY at the portion and banner level as circumstances warrant. Explicit foreign disclosure and release markings are not required on unclassified information. Follow internal agency procedures for the use of DISPLAY ONLY with unclassified information.

(U) Legacy documents (e.g., portions extracted, reintroduced into the working environment from a resting state):   Information marked as SECRET SENSITIVE DISPLAY ONLY, DISPLAY ONLY TO [LIST], FOR DISPLAY ONLY [LIST], or other legacy marking to denote a disclosure decision must not be used in a new product.   Any documents dated before publication of Register Version 4.1 that contain these markings should be referred to the originating agency prior to re-use.

(U) Derivative Use: (i.e., re-use of information in whole or in part in other intelligence products):   When the DISPLAY ONLY warning statement (noted below) is present on US classified intelligence information, derivative use of this information into other products, including other purposes, and other countries or international organizations is prohibited without prior authorization from the originating agency. Once authorization to use as a derivative source is received, remove the warning from the derived product.

(U) Warnings and Notices:   Information marked DISPLAY ONLY or when REL TO is used in conjunction with DISPLAY ONLY that is not authorized to be used as a derivative source into other products, must be marked with the following warning conspicuously located on the first page   –   top preferred:   “ (U) Derivative use of this DISPLAY ONLY or REL TO in conjunction with DISPLAY ONLY marked information into other products is prohibited without prior authorization from the originating agency. Disclosure of DISPLAY ONLY or REL TO in conjunction with DISPLAY ONLY information is not authorized for other purposes or for disclosure or release and disclosure to other countries, international organizations, or coalitions not specified in the banner line or portion mark. Removal of this warning is required once authorization is received by the originating agency.”

end page 164               UNCLASSIFIED

---
begin page 165               UNCLASSIFIED

(U)   Notional Example Page 1:  SECRET//DISPLAY ONLY AFG  [ Insert DISPLAY ONLY warning when derivative use is not authorized by the originator ]  (S//DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for DISPLAY ONLY Afghanistan (AFG). This portion is marked for training purposes only. (S//DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for DISPLAY ONLY AFG. This portion is marked for training purposes only. (S//DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for DISPLAY ONLY AFG. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//DISPLAY ONLY AFG

(U)   Notional Example Page 2 : SECRET//DISPLAY ONLY AFG  [ Insert DISPLAY ONLY warning when derivative use is not authorized by the originator ]  (S//DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for DISPLAY ONLY to Afghanistan (AFG). This portion is marked for training purposes only. (S//DISPLAY ONLY AFG, IRQ) This portion is classified SECRET and is authorized for DISPLAY ONLY to AFG and Iraq (IRQ). This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//DISPLAY ONLY AFG

end page 165               UNCLASSIFIED

---
begin page 166               UNCLASSIFIED

(U)   Notional Example Page 3 : SECRET//NOFORN  [ Insert DISPLAY ONLY warning when derivative use is not authorized by the originator ]  (S//DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for DISPLAY ONLY to Afghanistan (AFG). This portion is marked for training purposes only. (S//DISPLAY ONLY IRQ) This portion is classified SECRET and is authorized for DISPLAY ONLY to Iraq (IRQ). This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN

(U) Notional Example Page 4 : SECRET//DISPLAY ONLY IRQ  [ Insert DISPLAY ONLY warning when derivative use is not authorized by the originator ]  (S//REL TO USA, IRQ) This is the portion mark for a portion that is classified SECRET authorized for release to Iraq (IRQ). This portion is marked for training purposes only. (S//DISPLAY ONLY IRQ) This is the portion mark for a portion that is classified SECRET authorized for DISPLAY ONLY IRQ. This portion is marked for training purposes only.

(U) Note:   In this case, the roll-up to DISPLAY ONLY IRQ is the most restrictive marking and reflects that any US intelligence information approved for release to a given audience has automatically been approved for disclosure to that audience.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO implementing Directive and General Marking Guidance Section of this document for more information. SECRET//DISPLAY ONLY IRQ

end page 166               UNCLASSIFIED

---
begin page 167               UNCLASSIFIED

(U)   Notional Example Page 5 : SECRET//REL TO USA, IRQ/DISPLAY ONLY AFG  [ Insert DISPLAY ONLY warning when derivative use is not authorized by the originator ]  (S//REL TO USA, IRQ/DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for release to Iraq (IRQ) and DISPLAY ONLY to Afghanistan (AFG). This portion is marked for training purposes only. (S//REL TO USA, IRQ/DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for release to IRQ and DISPLAY ONLY to AFG. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO implementing Directive and General Marking Guidance Section of this document for more information. SECRET//REL TO USA , IRQ//DISPLAY ONLY AFG

(U)   Notional Example Page 6:  SECRET//NOFORN  [ Insert DISPLAY ONLY warning when derivative use is not authorized by the originator ]  (S//REL TO USA, AFG/DISPLAY ONLY IRQ) This portion is classified SECRET and is authorized for release to Afghanistan (AFG) and DISPLAY ONLY to Iraq (IRQ). This portion is marked for training purposes only. (S//REL TO USA, GBR/DISPLAY ONLY PAK) This portion is classified SECRET and is authorized for release to United Kingdom (GBR) and DISPLAY ONLY to PAK. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN

end page 167               UNCLASSIFIED

---
begin page 168               UNCLASSIFIED

(U)   Notional Example Page 7:  SECRET//NOFORN  [ Insert DISPLAY ONLY warning when derivative use is not authorized by the originator ]  (S//DISPLAY ONLY IRQ) This portion is classified SECRET and is authorized for DISPLAY ONLY to Iraq (IRQ). This portion is marked for training purposes only. (S//RELIDO) This portion is classified SECRET and the originator has explicitly deferred the foreign disclosure and release determination to a SFDRA. This portion is marked for training purposes only.

(U) Note:   The classification authority block is required on all US classified NSI. See the ISOO implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN

end page 168               UNCLASSIFIED


## ISM Enumeration Data

# ISM CVE Enumerations - capco-dissem-validator

**ISM-v2022-DEC Authorized Markings Reference**

## CVEnumISMDissem

| Value | Description |
|-------|-------------|
| `RS` | RISK SENSITIVE |
| `FOUO` | FOR OFFICIAL USE ONLY |
| `OC` | ORIGINATOR CONTROLLED |
| `OC-USGOV` | ORIGINATOR CONTROLLED US GOVERNMENT |
| `IMC` | CONTROLLED IMAGERY |
| `NF` | NOT RELEASABLE TO FOREIGN NATIONALS |
| `PR` | CAUTION-PROPRIETARY INFORMATION INVOLVED |
| `REL` | AUTHORIZED FOR RELEASE TO |
| `RELIDO` | RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL |
| `EYES` | EYES ONLY |
| `DSEN` | DEA SENSITIVE |
| `RAWFISA` | RAW FOREIGN INTELLIGENCE SURVEILLANCE ACT |
| `FISA` | FOREIGN INTELLIGENCE SURVEILLANCE ACT |
| `DISPLAYONLY` | AUTHORIZED FOR DISPLAY BUT NOT RELEASE TO |
| `EXEMPT_FROM_ICD501_DISCOVERY` | EXEMPT FROM ICD501 DISCOVERY |
| `WAIVED` | WAIVED |
| `AC` | Attorney-Client |
| `AWP` | Attorney-WP |
| `DL_ONLY` | DL ONLY |
| `FED_ONLY` | FED ONLY |
| `FEDCON` | FEDCON |
| `NOCON` | NOCON |

## CVEnumISMDissemIcrm

| Value | Description |
|-------|-------------|
| `RS` | RISK SENSITIVE |
| `FOUO` | FOR OFFICIAL USE ONLY |
| `OC` | ORIGINATOR CONTROLLED |
| `OC-USGOV` | ORIGINATOR CONTROLLED US GOVERNMENT |
| `IMC` | CONTROLLED IMAGERY |
| `NF` | NOT RELEASABLE TO FOREIGN NATIONALS |
| `PR` | CAUTION-PROPRIETARY INFORMATION INVOLVED |
| `REL` | AUTHORIZED FOR RELEASE TO |
| `RELIDO` | RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL |
| `EYES` | EYES ONLY |
| `DSEN` | DEA SENSITIVE |
| `RAWFISA` | RAW FOREIGN INTELLIGENCE SURVEILLANCE ACT |
| `FISA` | FOREIGN INTELLIGENCE SURVEILLANCE ACT |
| `DISPLAYONLY` | AUTHORIZED FOR DISPLAY BUT NOT RELEASE TO |
| `EXEMPT_FROM_ICD501_DISCOVERY` | EXEMPT FROM ICD501 DISCOVERY |
| `WAIVED` | WAIVED |

## CVEnumISMDissemCommingled

| Value | Description |
|-------|-------------|
| `RS` | RISK SENSITIVE |
| `OC` | ORIGINATOR CONTROLLED |
| `OC-USGOV` | ORIGINATOR CONTROLLED US GOVERNMENT |
| `IMC` | CONTROLLED IMAGERY |
| `NF` | NOT RELEASABLE TO FOREIGN NATIONALS |
| `PR` | CAUTION-PROPRIETARY INFORMATION INVOLVED |
| `REL` | AUTHORIZED FOR RELEASE TO |
| `RELIDO` | RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL |
| `EYES` | EYES ONLY |
| `DSEN` | DEA SENSITIVE |
| `RAWFISA` | RAW FOREIGN INTELLIGENCE SURVEILLANCE ACT |
| `FISA` | FOREIGN INTELLIGENCE SURVEILLANCE ACT |
| `DISPLAYONLY` | AUTHORIZED FOR DISPLAY BUT NOT RELEASE TO |
| `EXEMPT_FROM_ICD501_DISCOVERY` | EXEMPT FROM ICD501 DISCOVERY |
| `WAIVED` | WAIVED |
| `AC` | Attorney-Client |
| `AWP` | Attorney-WP |
| `DL_ONLY` | DL ONLY |
| `FED_ONLY` | FED ONLY |
| `FEDCON` | FEDCON |
| `NOCON` | NOCON |

## CVEnumISMDissemCui

| Value | Description |
|-------|-------------|
| `AC` | Attorney-Client |
| `AWP` | Attorney-WP |
| `DISPLAYONLY` | AUTHORIZED FOR DISPLAY BUT NOT RELEASE TO |
| `DL_ONLY` | DL ONLY |
| `EXEMPT_FROM_ICD501_DISCOVERY` | EXEMPT FROM ICD501 DISCOVERY |
| `FED_ONLY` | FED ONLY |
| `FEDCON` | FEDCON |
| `NF` | NOT RELEASABLE TO FOREIGN NATIONALS |
| `NOCON` | NOCON |
| `REL` | AUTHORIZED FOR RELEASE TO |
| `RELIDO` | RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL |


### Trigraphs

For trigraphs, see the generated `CVEnumISMCATRelTO` (.rnc/rng/xsd) enum

### Tetragraphs

For tetragraphs, see the generated `CVEnumISMCATTetragraph` (.rnc/rng/xsd) enum


## Validation Output Format

When validating, structure your response as:

```
## Validation Result: [PASS | FAIL | NEEDS_REVISION]

### Rule/Token: [identifier]

### Analysis:
- **Citation**: [CAPCO-2016 §X.Y page Z]
- **Expected**: [what the source says]
- **Found**: [what was submitted]
- **Status**: [compliant/non-compliant]

### Issues (if any):
- [Issue 1 with citation]
- [Issue 2 with citation]

### Recommended Fix:
[Corrected version with rationale]
```
