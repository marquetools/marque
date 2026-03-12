<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00087">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00087][Error] Classified USA documents having SBU-NF Data must have NF at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If IC Markings System Register and Manual rules do not apply to the
        document then the rule does not apply and the rule returns true. If any element has
        attribute @ism:nonICmarkings specified with a value containing [SBU-NF], does not have attribute
        @ism:excludeFromRollup set to true, and the resourceElement has attribute @ism:classification
        specified with a value other than [U], this rule ensures that the resourceElement has
        attribute @ism:disseminationControls specified with a value containing [NF]. 
    </sch:p>
    <sch:rule id="ISM-ID-00087-R1" context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]">
        <sch:assert test="if (not($ISM_USGOV_RESOURCE)) then true() else if (index-of($partNonICmarkings_tok, 'SBU-NF') &gt; 0 and not($bannerClassification = 'U')) then (index-of($bannerDisseminationControls_tok, 'NF') &gt; 0) else true()" flag="error" role="error">
            [ISM-ID-00087][Error] Classified USA documents having SBU-NF Data must have NF at the resource level. 
        </sch:assert>
    </sch:rule>
</sch:pattern>