<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00119">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00119][Error] If ISM_USIC_RESOURCE and 
        1. attribute @ism:classification is not [U]
        AND
        2. not ISM_710_FDR_EXEMPT
        AND
        3. attribute @ism:excludeFromRollup is not true
        AND
        4. attribute @ism:disseminationControls must contain one or more of 
            [DISPLAYONLY], [REL], [RELIDO], [EYES], or [NF].
        
        Human Readable: All classified NSI that does not claim exemption from
        ICD 710 mandatory Foreign Disclosure and Release must have an 
        appropriate foreign disclosure or release marking.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If IC Markings System Register and Manual rules do not apply to the document, or the document is exempt from mandatory
        foreign disclosure and release markings, or the resource is unclassified, or excludeFromRollup is true, 
        then the rule does not apply. Otherwise, this rule ensures that the attribute disseminationControls contains at least
        one of the values [DISPLAYONLY], [RELIDO], [REL], [EYES], or [NF].
    </sch:p>
    <sch:rule id="ISM-ID-00119-R1" context="*[@ism:* except (@ism:pocType | @ism:DESVersion | @ism:ISMCATCESVersion | @ism:unregisteredNoticeType)                        and $ISM_USIC_RESOURCE                        and util:contributesToRollup(.)                        and not($ISM_710_FDR_EXEMPT)                        and not(@ism:classification='U')]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY', 'RELIDO','REL','EYES', 'NF'))" flag="error" role="error">
            [ISM-ID-00119][Error] If ISM_USIC_RESOURCE and 
            1. attribute @ism:classification is not [U]
            AND
            2. not ISM_710_FDR_EXEMPT
            AND
            3. attribute @ism:excludeFromRollup is not true
            AND
            4. attribute @ism:disseminationControls must contain one or more of 
            [DISPLAYONLY], [REL], [RELIDO], [EYES], or [NF].
            
            Human Readable: All classified NSI that does not claim exemption from
            ICD 710 mandatory Foreign Disclosure and Release must have an 
            appropriate foreign disclosure or release marking.
        </sch:assert>
    </sch:rule>
</sch:pattern>