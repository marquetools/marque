<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?> 
<?schematron-phases phaseids="PORTION VALUECHECK STRUCTURECHECK BANNER"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00532">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="ruleText"> [ISM-ID-00532][Error] For all elements with @ism:SARIdentifier with tokens
        that include classification portion marks (e.g., DOD:TS:aaaa or DOD:C:bbbb), the value of
        the classification portion mark cannot be higher than @ism:classification on the same
        element. Human Readable: For @ism:SARIdentifier tokens that include classification portion
        marks in their values, the classification portion mark cannot be higher than the
        classification value. Note that some @ism:SARIdentifier tokens may not contain
        classification portion marks, e.g., DNI:kkkk; the rule does not apply to these tokens. </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA"
        class="codeDesc"> For all elements with @ism:SARIdentifier with tokens that include
        classification portion marks (e.g., DOD:TS:aaaa or DOD:C:bbbb), check the value of the
        classification portion mark, which is found between two colons ':' according to the regex
        for SARs. The logic uses the fact that if, for example, ':TS:' is found anywhere in
        @ism:SARIdentifier, then the classification of the element should be 'TS'. The rule logic is
        as follows. If there is ':TS:' the @ism:SARIdentifier, then @ism:classification must be
        'TS'. Otherwise, if there is ':S:' in the @ism:SARIdentifier, then @ism:classification must
        be 'S' or 'TS'. Otherwise, if there is ':C:' in the @ism:SARIdentifier, then
        @ism:classification must be 'C' or 'S' or 'TS'. Otherwise, according to the regex, there
        is no classification portion marking in any of the tokens in @ism:SARIdentifier, so do not check against @ism:classification. 
    </sch:p>
    <sch:rule id="ISM-ID-00532-R1" context="*[@ism:SARIdentifier]">
        <sch:assert
            test="if (contains(string(./@ism:SARIdentifier),':TS:')) then 
                 (if (normalize-space(string(./@ism:classification))='TS') then true() else false() ) 
            else if (contains(string(./@ism:SARIdentifier),':S:')) then
                 (if ((normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='TS')) then true()
                    else false() )
            else if (contains(string(./@ism:SARIdentifier),':C:')) then
                 (if ((normalize-space(string(./@ism:classification))='TS' or normalize-space(string(./@ism:classification))='S' 
                    or normalize-space(string(./@ism:classification))='C')) then true() else false() )
            else if (not(contains(./@ism:SARIdentifier,':TS:') or contains(string(./@ism:SARIdentifier),':S:') or
                    contains(./@ism:SARIdentifier,':C:'))) then true()                
            else false()" 
            flag="error" role="error">[ISM-ID-00532][Error] For all elements with ism:SARIdentifier with tokens
            that include classification portion marks (e.g., DOD:TS:aaaa or DOD:C:bbbb), the value of
            the classification portion mark cannot be higher than @ism:classification on the same
            element. Human Readable: For @ism:SARIdentifier tokens that include classification portion
            marks in their values, the classification portion mark cannot be higher than the
            classification value. Note that some @ism:SARIdentifier tokens may not contain
            classification portion marks, e.g., DNI:kkkk; the rule does not apply to these tokens. 
        </sch:assert>
    </sch:rule>
</sch:pattern>
