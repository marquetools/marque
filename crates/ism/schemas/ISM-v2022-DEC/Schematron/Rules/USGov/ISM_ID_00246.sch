<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00246">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00246][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
        [RD], [FRD], or [TFNI] then the ISM_RESOURCE_ELEMENT must have a @ism:declassException of [AEA] or [NATO-AEA].
        
        Human Readable: USA documents containing [RD], [FRD], or [TFNI] data must have declassException 
        containing [AEA] or [NATO-AEA] at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:atomicEnergyMarkings is specified
      with a value containing a token matching [RD], [FRD], or [TFNI], then this rule ensures that the 
      ISM_RESOURCE_ELEMENT has a @ism:declassException of [AEA] or [NATO-AEA].
    </sch:p>
    <sch:rule id="ISM-ID-00246-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD','FRD', 'TFNI'))]">
      <sch:assert test="util:containsAnyOfTheTokens(@ism:declassException, ('AEA', 'NATO-AEA'))" flag="error" role="error">
          [ISM-ID-00246][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
          [RD], [FRD], or [TFNI] then the ISM_RESOURCE_ELEMENT must have a @ism:declassException of [AEA] or [NATO-AEA].
          
          Human Readable: USA documents containing [RD], [FRD], or [TFNI] data must have declassException 
          containing [AEA] or [NATO-AEA] at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>