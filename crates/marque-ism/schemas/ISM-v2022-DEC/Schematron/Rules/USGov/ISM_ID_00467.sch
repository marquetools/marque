<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00467">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00467][Warning] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings
        contains one of the name tokens [RD] or [FRD], then [RD] and [FRD] SHOULD contain [NF].
        In order to release [RD] or [FRD] data to a foreign partner, ensure you have established a sharing
        agreement per the AEA. 
        
        Human Readable: A USA document containing RD and/or FRD data is usually NOFORN;
        ensure you have proper release authority per the AEA. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which specifies
        attribute @ism:atomicEnergyMarkings with a value containing one of the tokens [RD] or [FRD], this rule checks
        that attribute @ism:disseminationControls is specified with a value containing the token [NF]
        and gives a WARNING if there is no [NF].
    </sch:p>
    <sch:rule id="ISM-ID-00467-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD','FRD'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))" flag="warning" role="warning">
            [ISM-ID-00467][Warning] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings
            contains one of the name tokens [RD] or [FRD], then [RD] and [FRD] SHOULD contain [NF].
            In order to release [RD] or [FRD] data to a foreign partner, ensure you have established a sharing
            agreement per the AEA. 
            
            Human Readable: A USA document containing RD and/or FRD data is usually NOFORN;
            ensure you have proper release authority per the AEA. 
        </sch:assert>
    </sch:rule>
</sch:pattern>
