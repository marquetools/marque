<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER ROLLUP STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00176">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00176][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings has a name token containing [RD] or [FRD], 
        then attributes @ism:declassDate and @ism:declassEvent cannot be specified
        on the resourceElement.

        Human Readable: Automatic declassification of documents containing 
        RD or FRD information is prohibited. Attributes declassDate and 
        declassEvent cannot be used in the classification authority block when 
        RD or FRD is present.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	has attribute ism:atomicEnergyMarkings specified with a value containing
        the token [RD] or [FRD], this rule ensures that the resourceElement does not
    	have attributes ism:declassDate or ism:declassEvent specified.
    </sch:p>	
  <sch:rule id="ISM-ID-00176-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD'))]">
        <sch:assert test="not($ISM_RESOURCE_ELEMENT/@ism:declassDate or $ISM_RESOURCE_ELEMENT/@ism:declassEvent)" flag="error" role="error">
            [ISM-ID-00176][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:atomicEnergyMarkings has a name token containing [RD] or [FRD], 
            then attributes @ism:declassDate and @ism:declassEvent cannot be specified
            on the resourceElement.
            
            Human Readable: Automatic declassification of documents containing 
            RD or FRD information is prohibited. Attributes declassDate and 
            declassEvent cannot be used in the classification authority block when 
            RD or FRD is present.
        </sch:assert>

    </sch:rule>

</sch:pattern>