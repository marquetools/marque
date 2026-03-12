<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00376">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
	  	[ISM-ID-00376][Error] A portion using tetragraphs may not have a releasableTo 
	  	that is less restrictive than the releasability of any tetragraph or organization tokens used
	  	in the same portion’s releasableTo, displayOnlyTo, FGIsourceOpen, or FGIsourceProtected attributes.
	  	If a tetragraph XXXX in any of the attributes ownerProducer, releasableTo, displayOnlyTo, FGIsourceOpen, 
	  	or FGIsourceProtected is itself marked as ism:releasableTo in the Tetragraph Taxonomy, then see if all
	  	the countries that the portion is releasableTo are also countries that the tetragraph XXXX is releasableTo.  If not, error.  
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	Determine the set of releasableTo countries by determining, for each token in releasableTo, if it is a country code or tetragraph.
	  	If it is a tetragraph get the membership from ISMCAT Taxonomy and add the membership to the variable releasableToCountries; 
	  	otherwise, add the token to the variable releasableToCountries.  Then get the list of tetragraphs that appear in any of the 
	  	attributes @ism:ownerProducer, @ism:releasableTo, @ism:displayOnlyTo, @ism:FGIsourceOpen, or @ism:FGIsourceProtected and 
	  	put that list into the variable myTetras. Then determine if any of the tetragraph tokens in myTetras have releasability restrictions 
	  	themselves. If so, add those tetragraphs to the variable tetrasWithReleasableTo. Finally, determine if the releasability of any of the 
	  	tetragraph tokens in tetrasWithReleasableTo is more restrictive then the releasability of the portion, and if so,
	  	trigger the error message.
	</sch:p>
	  <sch:rule id="ISM-ID-00376-R1" context="*[@ism:ownerProducer]">
	  	<sch:let name="releasableToCountries" value="distinct-values(for $value in tokenize(normalize-space(@ism:releasableTo),' ') return      if(index-of($catt//catt:TetraToken,$value)&gt;0)      then util:tokenize(util:getTetragraphMembership($value))      else $value)"/>  	
	  	<sch:let name="myTetras" value="for $value in distinct-values(for $each in distinct-values((@ism:ownerProducer | @ism:releasableTo | @ism:displayOnlyTo | @ism:FGIsourceOpen | @ism:FGIsourceProtected)) return util:tokenize($each)) return if ($catt//catt:Tetragraph[catt:TetraToken=$value]) then $value else null"/>	  	
	  	<sch:let name="tetrasWithReleasableTo" value="distinct-values(for $value in $myTetras return        if($catt//catt:Tetragraph[catt:TetraToken=$value]/catt:TetraToken/@ism:releasableTo)      then $value        else null)"/>	  	
	  	<sch:let name="moreRestrictiveTetras" value="for $tetra in $tetrasWithReleasableTo return       if (every $value in $releasableToCountries satisfies index-of(distinct-values(util:tokenize(util:getTetragraphReleasability($tetra))),$value)&gt;0)        then null else $tetra"/>	  	
		    <sch:assert test="empty($moreRestrictiveTetras)" flag="error" role="error">
		    	[ISM-ID-00376][Error] A portion using tetragraphs may not have a releasableTo 
		    	that is less restrictive than the releasability of any tetragraph or organization tokens used
		    	in the same portion’s releasableTo, displayOnlyTo, FGIsourceOpen, or FGIsourceProtected attributes.
		    	If a tetragraph XXXX in any of the attributes ownerProducer, releasableTo, displayOnlyTo, FGIsourceOpen, 
		    	or FGIsourceProtected is itself marked as ism:releasableTo in the Tetragraph Taxonomy, then see if all
		    	the countries that the portion is releasableTo are also countries that the tetragraph XXXX is releasableTo.  If not, error. 
		    	The following tetragraphs have a more restrictive releasability than the portion: 
		    	<sch:value-of select="string-join($moreRestrictiveTetras,', ')"/>
		</sch:assert>
	  	<sch:assert test="exists($catt//catt:Tetragraphs)" flag="error" role="error">ISMCAT Taxonomy does not exist!</sch:assert>
	  </sch:rule>
</sch:pattern>