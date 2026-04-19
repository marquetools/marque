<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00245">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00245][Error] If ISM_USGOV_RESOURCE and:
        1. No element without @ism:excludeFromRollup=true() in the document has the attribute @ism:atomicEnergyMarkings containing [RD-CNWDI]
        AND
        2. Any element without @ism:excludeFromRollup=true() in the document has the attribute @ism:noticeType containing [CNWDI]
        and not the attribute @ism:externalNotice with a value of [true].
        
        Human Readable: USA documents containing an CNWDI notice must also have RD-CNWDI data.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE, for each element which meets
      ISM_CONTRIBUTES and specifies attribute @ism:noticeType with a value
      containing the token [CNWDI] and not the attribute @ism:externalNotice with a value of [true], 
      then this rule ensures that some element in the document specifies attribute @ism:atomicEnergyMarkings with a value
      containing the token [RD-CNWDI].
    </sch:p>
    <sch:rule id="ISM-ID-00245-R1" context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and (util:containsAnyOfTheTokens(@ism:noticeType, ('CNWDI'))) and not (@ism:externalNotice=true())]">
        <sch:assert test="index-of($partAtomicEnergyMarkings_tok, 'RD-CNWDI')&gt;0" flag="error" role="error">
            [ISM-ID-00245][Error] If ISM_USGOV_RESOURCE and:
            1. No element without @ism:excludeFromRollup=true() in the document has the attribute @ism:atomicEnergyMarkings containing [RD-CNWDI]
            AND
            2. Any element without @ism:excludeFromRollup=true() in the document has the attribute @ism:noticeType containing [CNWDI]
            and not the attribute @ism:externalNotice with a value of [true].
            
            Human Readable: USA documents containing an CNWDI notice must also have RD-CNWDI data.
        </sch:assert>
    </sch:rule>
</sch:pattern>