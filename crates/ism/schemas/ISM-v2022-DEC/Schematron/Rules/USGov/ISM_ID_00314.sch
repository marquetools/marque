<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00314">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00314][Error] If @ism:nonICmarkings contains the token [XD] then the 
        attribute @ism:disseminationControls must contain [NF].
        
        Human Readable: EXDIS data must be marked NOFORN.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the @ism:nonICmarkings contains the ND token, then check that the @ism:disseminationControls
        attribute must have NF specified.
    </sch:p>
    <sch:rule id="ISM-ID-00314-R1" context="*[util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))" flag="error" role="error">
            [ISM-ID-00314][Error] If @ism:nonICmarkings contains the token [XD] then the 
            attribute @ism:disseminationControls must contain [NF].
            
            Human Readable: EXDIS data must be marked NOFORN.
        </sch:assert>
    </sch:rule>
</sch:pattern>